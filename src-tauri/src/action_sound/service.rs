use std::collections::VecDeque;
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::action::SoundSettings;
use crate::error::{AppError, AppResult};

use super::{is_known_id, samples, wave_out::WaveOut};

const RUNTIME_VOICES: usize = 4;

pub(super) trait Voice {
    fn finished(&self) -> bool;
}

pub(super) trait Sink {
    fn play(&mut self, samples: Vec<i16>) -> Result<Box<dyn Voice>, String>;
}

struct Request {
    id: String,
    volume: u8,
    response: mpsc::Sender<Result<(), String>>,
}

#[derive(Default)]
struct Mailbox {
    preview: Option<Request>,
    runtime: VecDeque<(String, u8)>,
    shutdown: bool,
}

impl Mailbox {
    fn replace_preview(&mut self, request: Request) {
        if let Some(previous) = self.preview.replace(request) {
            // Superseded requests have successfully yielded to the latest value.
            let _ = previous.response.send(Ok(()));
        }
    }

    fn push_runtime(&mut self, id: String, volume: u8) {
        if self.runtime.len() == RUNTIME_VOICES {
            self.runtime.pop_front();
        }
        self.runtime.push_back((id, volume));
    }
}

#[derive(Default)]
struct Shared {
    mailbox: Mutex<Mailbox>,
    wake: Condvar,
}

struct Owner {
    shared: Arc<Shared>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl Owner {
    fn shutdown(&self) {
        {
            let mut mailbox = self.shared.mailbox.lock().unwrap();
            mailbox.shutdown = true;
            self.shared.wake.notify_one();
        }
        if let Some(worker) = self.worker.lock().unwrap().take() {
            let _ = worker.join();
        }
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[derive(Clone)]
pub struct AudioService(Arc<Owner>);

impl AudioService {
    pub fn start() -> Self {
        Self::with_sink(|| WaveOut)
    }

    fn with_sink<S: Sink + 'static>(factory: impl FnOnce() -> S + Send + 'static) -> Self {
        let shared = Arc::new(Shared::default());
        let worker_shared = Arc::clone(&shared);
        let worker = thread::spawn(move || run(worker_shared, factory()));
        Self(Arc::new(Owner {
            shared,
            worker: Mutex::new(Some(worker)),
        }))
    }

    pub fn preview(&self, id: &str, volume_percent: u8) -> AppResult<()> {
        if !is_known_id(id) {
            return Err(AppError::Validation("Неизвестный звук действия".into()));
        }
        if volume_percent > 100 {
            return Err(AppError::Validation(
                "Громкость должна быть от 0 до 100%".into(),
            ));
        }
        self.request(id, volume_percent)
    }

    pub fn stop_preview(&self) -> AppResult<()> {
        self.request("", 0)
    }

    fn request(&self, id: &str, volume: u8) -> AppResult<()> {
        let (response, result) = mpsc::channel();
        {
            let mut mailbox = self.0.shared.mailbox.lock().unwrap();
            if mailbox.shutdown {
                return Err(AppError::Audio("Аудиосервис остановлен".into()));
            }
            mailbox.replace_preview(Request {
                id: id.into(),
                volume,
                response,
            });
            self.0.shared.wake.notify_one();
        }
        result
            .recv()
            .unwrap_or_else(|_| Err("Аудиосервис остановлен".into()))
            .map_err(AppError::Audio)
    }

    /// Only a bounded mailbox insertion runs on the executor. Device work stays on the worker.
    pub fn play_runtime(&self, settings: &SoundSettings) {
        let Some(id) = &settings.id else {
            return;
        };
        if settings.volume_percent == 0 {
            return;
        }
        if !is_known_id(id) || settings.volume_percent > 100 {
            eprintln!("Invalid action sound settings");
            return;
        }
        let mut mailbox = self.0.shared.mailbox.lock().unwrap();
        if mailbox.shutdown {
            return;
        }
        mailbox.push_runtime(id.clone(), settings.volume_percent);
        self.0.shared.wake.notify_one();
    }

    /// Explicit app-exit cleanup even when an executor still owns a service clone.
    pub fn shutdown(&self) {
        self.0.shutdown();
    }

    #[cfg(test)]
    pub(crate) fn unavailable_for_test() -> Self {
        struct Unavailable;
        impl Sink for Unavailable {
            fn play(&mut self, _: Vec<i16>) -> Result<Box<dyn Voice>, String> {
                Err("device unavailable".into())
            }
        }
        Self::with_sink(|| Unavailable)
    }
}

fn run(shared: Arc<Shared>, mut sink: impl Sink) {
    let mut preview: Option<Box<dyn Voice>> = None;
    let mut runtime: VecDeque<Box<dyn Voice>> = VecDeque::new();
    loop {
        let (request, pending) = {
            let mut mailbox = shared.mailbox.lock().unwrap();
            if !mailbox.shutdown && mailbox.preview.is_none() && mailbox.runtime.is_empty() {
                if preview.is_some() || !runtime.is_empty() {
                    mailbox = shared
                        .wake
                        .wait_timeout(mailbox, Duration::from_millis(10))
                        .unwrap()
                        .0;
                } else {
                    mailbox = shared.wake.wait(mailbox).unwrap();
                }
            }
            if mailbox.shutdown {
                if let Some(request) = mailbox.preview.take() {
                    let _ = request.response.send(Err("Аудиосервис остановлен".into()));
                }
                break;
            }
            (mailbox.preview.take(), std::mem::take(&mut mailbox.runtime))
        };
        if preview.as_ref().is_some_and(|voice| voice.finished()) {
            preview = None;
        }
        runtime.retain(|voice| !voice.finished());
        if let Some(request) = request {
            // Dropping the old voice resets and releases it before starting another.
            preview = None;
            let result = if request.volume == 0 {
                Ok(())
            } else {
                sink.play(samples(&request.id, request.volume).unwrap())
                    .map(|voice| {
                        preview = Some(voice);
                    })
            };
            let _ = request.response.send(result);
        }
        for (id, volume) in pending {
            if runtime.len() == RUNTIME_VOICES {
                runtime.pop_front();
            }
            match sink.play(samples(&id, volume).unwrap()) {
                Ok(voice) => runtime.push_back(voice),
                Err(error) => eprintln!("Action sound playback failed: {error}"),
            }
        }
    }
    // Owned voices reset/unprepare/close when dropped, including shutdown.
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    #[derive(Default)]
    struct Record {
        active: AtomicUsize,
        max: AtomicUsize,
        dropped: AtomicUsize,
        started: Mutex<Vec<Vec<i16>>>,
        fail: AtomicBool,
        done: AtomicBool,
        failed_attempts: Mutex<usize>,
        failure: Condvar,
    }
    struct FakeSink(Arc<Record>);
    struct FakeVoice(Arc<Record>);
    impl Voice for FakeVoice {
        fn finished(&self) -> bool {
            self.0.done.load(Ordering::SeqCst)
        }
    }
    impl Drop for FakeVoice {
        fn drop(&mut self) {
            self.0.active.fetch_sub(1, Ordering::SeqCst);
            self.0.dropped.fetch_add(1, Ordering::SeqCst);
        }
    }
    impl Sink for FakeSink {
        fn play(&mut self, samples: Vec<i16>) -> Result<Box<dyn Voice>, String> {
            if self.0.fail.load(Ordering::SeqCst) {
                *self.0.failed_attempts.lock().unwrap() += 1;
                self.0.failure.notify_all();
                return Err("device unavailable".into());
            }
            self.0.started.lock().unwrap().push(samples);
            let active = self.0.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.0.max.fetch_max(active, Ordering::SeqCst);
            Ok(Box::new(FakeVoice(Arc::clone(&self.0))))
        }
    }
    fn fake() -> (AudioService, Arc<Record>) {
        let record = Arc::new(Record::default());
        let sink_record = record.clone();
        (
            AudioService::with_sink(move || FakeSink(sink_record)),
            record,
        )
    }

    #[test]
    fn replacing_preview_stopping_and_zero_release_old_voice() {
        let (service, record) = fake();
        service.preview("soft-ping", 50).unwrap();
        service.preview("bright-bell", 35).unwrap();
        assert_eq!(record.max.load(Ordering::SeqCst), 1);
        assert_eq!(record.dropped.load(Ordering::SeqCst), 1);
        assert_eq!(
            record.started.lock().unwrap()[1],
            samples("bright-bell", 35).unwrap()
        );
        service.preview("soft-ping", 0).unwrap();
        assert_eq!(record.active.load(Ordering::SeqCst), 0);
        service.preview("soft-ping", 50).unwrap();
        service.stop_preview().unwrap();
        assert_eq!(record.active.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn invalid_preview_input_never_reaches_audio_sink() {
        let (service, record) = fake();
        assert!(matches!(
            service.preview("unknown-sound", 50),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            service.preview("soft-ping", 101),
            Err(AppError::Validation(_))
        ));
        service.shutdown();
        assert!(record.started.lock().unwrap().is_empty());
        assert_eq!(*record.failed_attempts.lock().unwrap(), 0);
        assert_eq!(record.active.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn pending_preview_and_runtime_mailboxes_are_bounded_latest_wins() {
        let mut mailbox = Mailbox::default();
        let (first, first_result) = mpsc::channel();
        let (last, _) = mpsc::channel();
        mailbox.replace_preview(Request {
            id: "soft-ping".into(),
            volume: 10,
            response: first,
        });
        mailbox.replace_preview(Request {
            id: "bright-bell".into(),
            volume: 90,
            response: last,
        });
        assert_eq!(first_result.recv().unwrap(), Ok(()));
        assert_eq!(mailbox.preview.as_ref().unwrap().id, "bright-bell");
        assert_eq!(mailbox.preview.as_ref().unwrap().volume, 90);
        for volume in 0..100 {
            mailbox.push_runtime("soft-ping".into(), volume);
        }
        assert_eq!(mailbox.runtime.len(), RUNTIME_VOICES);
        assert_eq!(mailbox.runtime.front().unwrap().1, 96);
        assert_eq!(mailbox.runtime.back().unwrap().1, 99);
    }

    #[test]
    fn busy_worker_plays_only_latest_pending_preview_and_runtime_submission_returns() {
        struct BlockingSink {
            fake: FakeSink,
            entered: mpsc::Sender<()>,
            release: mpsc::Receiver<()>,
            first: bool,
        }
        impl Sink for BlockingSink {
            fn play(&mut self, samples: Vec<i16>) -> Result<Box<dyn Voice>, String> {
                if self.first {
                    self.first = false;
                    self.entered.send(()).unwrap();
                    self.release.recv().unwrap();
                }
                self.fake.play(samples)
            }
        }
        let record = Arc::new(Record::default());
        let sink_record = record.clone();
        let (entered, entry) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let service = AudioService::with_sink(move || BlockingSink {
            fake: FakeSink(sink_record),
            entered,
            release: gate,
            first: true,
        });
        let first_service = service.clone();
        let first = thread::spawn(move || first_service.preview("soft-ping", 10));
        entry.recv_timeout(Duration::from_secs(2)).unwrap();
        // Queue behind the blocked device using the same bounded request mechanism.
        let (old_response, old_result) = mpsc::channel();
        let (new_response, new_result) = mpsc::channel();
        {
            let mut mailbox = service.0.shared.mailbox.lock().unwrap();
            mailbox.replace_preview(Request {
                id: "bright-bell".into(),
                volume: 20,
                response: old_response,
            });
            mailbox.replace_preview(Request {
                id: "crystal-drop".into(),
                volume: 30,
                response: new_response,
            });
        }
        assert_eq!(
            old_result.recv_timeout(Duration::from_secs(2)).unwrap(),
            Ok(())
        );
        // The audio device is blocked here: runtime insertion still returns immediately.
        service.play_runtime(&SoundSettings {
            id: Some("low-pulse".into()),
            volume_percent: 40,
        });
        assert_eq!(service.0.shared.mailbox.lock().unwrap().runtime.len(), 1);
        release.send(()).unwrap();
        first.join().unwrap().unwrap();
        assert_eq!(
            new_result.recv_timeout(Duration::from_secs(2)).unwrap(),
            Ok(())
        );
        service.shutdown();
        let played = record.started.lock().unwrap();
        assert_eq!(played.len(), 3);
        assert_eq!(played[0], samples("soft-ping", 10).unwrap());
        assert_eq!(played[1], samples("crystal-drop", 30).unwrap());
        assert_eq!(played[2], samples("low-pulse", 40).unwrap());
        assert_eq!(record.active.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn device_error_is_returned_to_preview_and_runtime_can_recover() {
        let (service, record) = fake();
        record.fail.store(true, Ordering::SeqCst);
        assert!(
            matches!(service.preview("soft-ping", 50), Err(AppError::Audio(message)) if message == "device unavailable")
        );
        service.play_runtime(&SoundSettings {
            id: Some("soft-ping".into()),
            volume_percent: 50,
        });
        let attempts = record.failed_attempts.lock().unwrap();
        let (attempts, wait) = record
            .failure
            .wait_timeout_while(attempts, Duration::from_secs(2), |count| *count < 2)
            .unwrap();
        assert!(!wait.timed_out());
        assert_eq!(*attempts, 2);
        drop(attempts);
        // The runtime attempt has failed on the worker; it remains alive for recovery.
        record.fail.store(false, Ordering::SeqCst);
        service.preview("soft-ping", 50).unwrap();
        service.shutdown();
        assert_eq!(record.active.load(Ordering::SeqCst), 0);
        assert!(service.preview("soft-ping", 50).is_err());
    }

    #[test]
    fn runtime_limit_does_not_block_executor_and_shutdown_releases_every_voice() {
        let (service, record) = fake();
        for _ in 0..100 {
            service.play_runtime(&SoundSettings {
                id: Some("soft-ping".into()),
                volume_percent: 50,
            });
        }
        service.preview("bright-bell", 50).unwrap();
        service.shutdown();
        assert!(record.max.load(Ordering::SeqCst) <= RUNTIME_VOICES + 1);
        assert_eq!(record.active.load(Ordering::SeqCst), 0);
        assert_eq!(
            record.dropped.load(Ordering::SeqCst),
            record.started.lock().unwrap().len()
        );
    }

    #[test]
    fn finished_voices_are_released_and_muted_runtime_creates_none() {
        let (service, record) = fake();
        service.play_runtime(&SoundSettings {
            id: None,
            volume_percent: 50,
        });
        service.play_runtime(&SoundSettings {
            id: Some("soft-ping".into()),
            volume_percent: 0,
        });
        service.preview("soft-ping", 50).unwrap();
        record.done.store(true, Ordering::SeqCst);
        service.stop_preview().unwrap();
        assert_eq!(record.started.lock().unwrap().len(), 1);
        assert_eq!(record.active.load(Ordering::SeqCst), 0);
    }
}
