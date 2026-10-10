//! Camera shutter and selection tick synthesized during hidden editor preparation.

use std::sync::OnceLock;

use windows::core::PCWSTR;
use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY};

const SAMPLE_RATE: u32 = 44_100;
const DURATION_MS: u32 = 150;
/// Start of each click and its decay time constant, in milliseconds.
const CLICKS: [(f32, f32); 2] = [(0.0, 9.0), (70.0, 14.0)];

static WAV: OnceLock<Vec<u8>> = OnceLock::new();
static SELECTION_WAV: OnceLock<Vec<u8>> = OnceLock::new();
const SELECTION_DURATION_MS: u32 = 90;

/// Builds the shutter buffer ahead of the snapshot. Playback is [`play`].
pub fn prepare() {
    let _ = prepared_wav();
    let _ = SELECTION_WAV.get_or_init(selection_wav);
}

pub fn play() {
    let wav = prepared_wav();
    // SND_ASYNC keeps reading the buffer after returning; the static outlives playback.
    unsafe {
        let _ = PlaySoundW(PCWSTR(wav.as_ptr().cast()), None, SND_MEMORY | SND_ASYNC);
    }
}

/// A clear, short acknowledgment of an accepted point or area.
/// Preparation already initialized this static buffer; selection has no synthesis or file IO.
pub fn play_selection() {
    if let Some(wav) = SELECTION_WAV.get() {
        unsafe {
            let _ = PlaySoundW(PCWSTR(wav.as_ptr().cast()), None, SND_MEMORY | SND_ASYNC);
        }
    }
}

fn selection_wav() -> Vec<u8> {
    let sample_count = SAMPLE_RATE * SELECTION_DURATION_MS / 1000;
    let samples = (0..sample_count).map(|index| {
        let time = index as f32 / SAMPLE_RATE as f32;
        let attack = (time / 0.002).min(1.0);
        let release = ((time - 0.010) / 0.080).clamp(0.0, 1.0);
        let envelope = attack * (1.0 + (std::f32::consts::PI * release).cos()) * 0.5;
        let tone = 0.7 * (std::f32::consts::TAU * 1_100.0 * time).sin()
            + 0.3 * (std::f32::consts::TAU * 1_650.0 * time).sin();
        (tone * envelope * 0.7 * i16::MAX as f32) as i16
    });
    wav_mono_16(samples, sample_count)
}

fn prepared_wav() -> &'static [u8] {
    WAV.get_or_init(shutter_wav)
}

fn shutter_wav() -> Vec<u8> {
    let sample_count = SAMPLE_RATE * DURATION_MS / 1000;
    let mut noise = 0x2545_f491_u32;
    let samples = (0..sample_count).map(|index| {
        noise ^= noise << 13;
        noise ^= noise >> 17;
        noise ^= noise << 5;
        let white = noise as f32 / u32::MAX as f32 * 2.0 - 1.0;

        let time_ms = index as f32 * 1000.0 / SAMPLE_RATE as f32;
        let envelope: f32 = CLICKS
            .iter()
            .filter(|&&(start, _)| time_ms >= start)
            .map(|&(start, decay)| (-(time_ms - start) / decay).exp())
            .sum();
        (white * envelope.min(1.0) * 0.6 * i16::MAX as f32) as i16
    });
    wav_mono_16(samples, sample_count)
}

fn wav_mono_16(samples: impl Iterator<Item = i16>, sample_count: u32) -> Vec<u8> {
    let data_len = sample_count * 2;
    let mut wav = Vec::with_capacity(44 + data_len as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    wav.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes()); // byte rate
    wav.extend_from_slice(&2u16.to_le_bytes()); // block align
    wav.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        wav.extend_from_slice(&sample.to_le_bytes());
    }
    wav
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u16_at(bytes: &[u8], offset: usize) -> u16 {
        u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
    }

    fn u32_at(bytes: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
    }

    #[test]
    fn prepare_matches_synthesis_without_an_audio_device() {
        prepare();
        assert_eq!(prepared_wav(), shutter_wav().as_slice());
        assert_eq!(SELECTION_WAV.get().unwrap(), &selection_wav());
    }

    #[test]
    fn selection_is_a_distinct_audible_90_ms_tick() {
        let wav = selection_wav();
        assert_eq!(u32_at(&wav, 24), SAMPLE_RATE);
        assert_eq!(u32_at(&wav, 40), SAMPLE_RATE * 90 / 1000 * 2);
        let samples: Vec<i16> = wav[44..]
            .chunks_exact(2)
            .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        let peak = samples
            .iter()
            .map(|sample| sample.unsigned_abs())
            .max()
            .unwrap();
        assert!((15_000..25_000).contains(&peak));
        let audible = &samples[..(SAMPLE_RATE * 30 / 1000) as usize];
        let rms = (audible
            .iter()
            .map(|&sample| (sample as f64).powi(2))
            .sum::<f64>()
            / audible.len() as f64)
            .sqrt();
        assert!(rms > 9_000.0, "confirmation must sustain an audible level");
        assert!(samples[samples.len() - 200..]
            .iter()
            .all(|sample| sample.unsigned_abs() < 300));
        assert_ne!(wav, shutter_wav());
    }

    #[test]
    fn builds_150_ms_mono_16_bit_pcm_wave() {
        let wav = shutter_wav();
        let data_len = 44_100 * 150 / 1000 * 2;

        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(u32_at(&wav, 4) as usize, wav.len() - 8);
        assert_eq!(&wav[8..16], b"WAVEfmt ");
        assert_eq!(u32_at(&wav, 16), 16);
        assert_eq!(u16_at(&wav, 20), 1);
        assert_eq!(u16_at(&wav, 22), 1);
        assert_eq!(u32_at(&wav, 24), 44_100);
        assert_eq!(u32_at(&wav, 28), 88_200);
        assert_eq!(u16_at(&wav, 32), 2);
        assert_eq!(u16_at(&wav, 34), 16);
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(u32_at(&wav, 40), data_len);
        assert_eq!(wav.len(), 44 + data_len as usize);
    }

    #[test]
    fn decays_to_near_silence_by_the_end() {
        let wav = shutter_wav();
        let samples: Vec<i16> = wav[44..]
            .chunks_exact(2)
            .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        let loudest = |range: &[i16]| range.iter().map(|s| s.unsigned_abs()).max().unwrap();

        assert!(loudest(&samples[..200]) > 5_000);
        assert!(loudest(&samples[samples.len() - 200..]) < 1_000);
    }

    /// Listen for a shutter click: `cargo test -- --ignored plays_shutter`.
    #[test]
    #[ignore]
    fn plays_shutter() {
        play();
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}
