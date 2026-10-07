use std::mem::size_of;

use windows::core::PSTR;
use windows::Win32::Media::Audio::{
    waveOutClose, waveOutOpen, waveOutPrepareHeader, waveOutReset, waveOutUnprepareHeader,
    waveOutWrite, CALLBACK_NULL, HWAVEOUT, WAVEFORMATEX, WAVEHDR, WAVERR_STILLPLAYING, WAVE_MAPPER,
    WHDR_DONE,
};

use super::service::{Sink, Voice};
use super::SAMPLE_RATE;

pub(super) struct WaveOut;

struct NativeVoice {
    device: HWAVEOUT,
    // Both allocations stay at the same addresses until the driver releases the header.
    samples: Option<Vec<i16>>,
    header: Option<Box<WAVEHDR>>,
}

impl Sink for WaveOut {
    fn play(&mut self, mut samples: Vec<i16>) -> Result<Box<dyn Voice>, String> {
        let format = WAVEFORMATEX {
            wFormatTag: 1,
            nChannels: 1,
            nSamplesPerSec: SAMPLE_RATE,
            nAvgBytesPerSec: SAMPLE_RATE * 2,
            nBlockAlign: 2,
            wBitsPerSample: 16,
            cbSize: 0,
        };
        let mut device = HWAVEOUT::default();
        let result = unsafe {
            waveOutOpen(
                Some(&mut device),
                WAVE_MAPPER,
                &format,
                None,
                None,
                CALLBACK_NULL,
            )
        };
        check("open", result)?;
        let mut header = Box::new(WAVEHDR {
            lpData: PSTR(samples.as_mut_ptr().cast()),
            dwBufferLength: (samples.len() * size_of::<i16>()) as u32,
            ..Default::default()
        });
        let result =
            unsafe { waveOutPrepareHeader(device, &mut *header, size_of::<WAVEHDR>() as u32) };
        if result != 0 {
            unsafe {
                waveOutClose(device);
            }
            return Err(format!(
                "Не удалось подготовить звук (waveOut prepare: {result})"
            ));
        }
        let mut voice = NativeVoice {
            device,
            samples: Some(samples),
            header: Some(header),
        };
        let result = unsafe {
            waveOutWrite(
                device,
                &mut **voice.header.as_mut().unwrap(),
                size_of::<WAVEHDR>() as u32,
            )
        };
        check("write", result)?;
        Ok(Box::new(voice))
    }
}

fn check(operation: &str, result: u32) -> Result<(), String> {
    if result == 0 {
        Ok(())
    } else {
        Err(format!(
            "Не удалось воспроизвести звук (waveOut {operation}: {result})"
        ))
    }
}

impl Voice for NativeVoice {
    fn finished(&self) -> bool {
        // The multimedia driver updates this field asynchronously; do not cache the read.
        let header = &**self.header.as_ref().unwrap();
        unsafe { std::ptr::read_volatile(std::ptr::addr_of!(header.dwFlags)) & WHDR_DONE != 0 }
    }
}

// Kept separate so reset/unprepare/close ordering can be verified without an audio device.
fn release(
    mut reset: impl FnMut() -> u32,
    mut unprepare: impl FnMut() -> u32,
    mut close: impl FnMut() -> u32,
) -> bool {
    let result = reset();
    if result != 0 {
        eprintln!("waveOut reset failed: {result}");
    }
    for attempt in 0..=20 {
        let result = unprepare();
        if result == WAVERR_STILLPLAYING {
            if attempt == 20 {
                eprintln!(
                    "waveOut header remained in use after reset; retaining driver-owned memory"
                );
                return false;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
            continue;
        }
        if result != 0 {
            eprintln!("waveOut unprepare failed: {result}; retaining driver-owned memory");
            return false;
        }
        let result = close();
        if result != 0 {
            eprintln!("waveOut close failed: {result}");
        }
        return true;
    }
    unreachable!()
}

impl Drop for NativeVoice {
    fn drop(&mut self) {
        let header = &mut **self.header.as_mut().unwrap() as *mut WAVEHDR;
        let device = self.device;
        let released = release(
            || unsafe { waveOutReset(device) },
            || unsafe { waveOutUnprepareHeader(device, header, size_of::<WAVEHDR>() as u32) },
            || unsafe { waveOutClose(device) },
        );
        if !released {
            // A broken driver must never be handed freed memory. This exceptional leak
            // is preferable to freeing an allocation that is still owned by waveOut.
            std::mem::forget(self.samples.take().unwrap());
            std::mem::forget(self.header.take().unwrap());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    #[test]
    fn reset_then_unprepare_then_close_before_freeing_buffers() {
        let operations = RefCell::new(Vec::new());
        let calls = Cell::new(0);
        assert!(release(
            || {
                operations.borrow_mut().push("reset");
                0
            },
            || {
                operations.borrow_mut().push("unprepare");
                calls.set(calls.get() + 1);
                if calls.get() == 1 {
                    WAVERR_STILLPLAYING
                } else {
                    0
                }
            },
            || {
                operations.borrow_mut().push("close");
                0
            },
        ));
        assert_eq!(
            *operations.borrow(),
            ["reset", "unprepare", "unprepare", "close"]
        );
    }

    #[test]
    fn failed_release_keeps_driver_owned_buffer_alive() {
        let closed = Cell::new(false);
        assert!(!release(
            || 1,
            || 1,
            || {
                closed.set(true);
                0
            }
        ));
        assert!(!closed.get());
    }

    #[test]
    fn stuck_driver_cannot_wait_forever_or_close_in_use_header() {
        let calls = Cell::new(0);
        assert!(!release(
            || 1,
            || {
                calls.set(calls.get() + 1);
                WAVERR_STILLPLAYING
            },
            || panic!("still-owned header cannot be closed")
        ));
        assert_eq!(calls.get(), 21);
    }
}
