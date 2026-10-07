//! Offline action cues: stable catalog, cached mono PCM, and bounded audio worker.
//! See `action_sound/README.md` for synthesis and native lifetime guarantees.

mod service;
mod wave_out;

pub use service::AudioService;

use std::f64::consts::TAU;
use std::sync::OnceLock;

use serde::Serialize;

pub(super) const SAMPLE_RATE: u32 = 44_100;

#[derive(Clone, Copy, Debug, Serialize)]
pub struct SoundInfo {
    pub id: &'static str,
    pub name: &'static str,
}

const CATALOG: [SoundInfo; 20] = [
    SoundInfo {
        id: "soft-ping",
        name: "Soft Ping",
    },
    SoundInfo {
        id: "bright-bell",
        name: "Bright Bell",
    },
    SoundInfo {
        id: "crystal-drop",
        name: "Crystal Drop",
    },
    SoundInfo {
        id: "gentle-chime",
        name: "Gentle Chime",
    },
    SoundInfo {
        id: "double-tap",
        name: "Double Tap",
    },
    SoundInfo {
        id: "triple-beep",
        name: "Triple Beep",
    },
    SoundInfo {
        id: "rising-spark",
        name: "Rising Spark",
    },
    SoundInfo {
        id: "falling-star",
        name: "Falling Star",
    },
    SoundInfo {
        id: "digital-pop",
        name: "Digital Pop",
    },
    SoundInfo {
        id: "wooden-click",
        name: "Wooden Click",
    },
    SoundInfo {
        id: "bubble-pop",
        name: "Bubble Pop",
    },
    SoundInfo {
        id: "glass-tick",
        name: "Glass Tick",
    },
    SoundInfo {
        id: "marimba-note",
        name: "Marimba Note",
    },
    SoundInfo {
        id: "low-pulse",
        name: "Low Pulse",
    },
    SoundInfo {
        id: "radar-blip",
        name: "Radar Blip",
    },
    SoundInfo {
        id: "echo-pluck",
        name: "Echo Pluck",
    },
    SoundInfo {
        id: "tiny-gong",
        name: "Tiny Gong",
    },
    SoundInfo {
        id: "arcade-coin",
        name: "Arcade Coin",
    },
    SoundInfo {
        id: "success-chord",
        name: "Success Chord",
    },
    SoundInfo {
        id: "alert-chirp",
        name: "Alert Chirp",
    },
];

pub fn catalog() -> &'static [SoundInfo] {
    &CATALOG
}

pub fn is_known_id(id: &str) -> bool {
    CATALOG.iter().any(|sound| sound.id == id)
}

fn cached_samples(id: &str) -> Option<&'static [i16]> {
    static CACHE: [OnceLock<Vec<i16>>; 20] = [const { OnceLock::new() }; 20];
    let index = CATALOG.iter().position(|sound| sound.id == id)?;
    Some(CACHE[index].get_or_init(|| synthesize(index)))
}

pub(super) fn samples(id: &str, volume: u8) -> Option<Vec<i16>> {
    Some(
        cached_samples(id)?
            .iter()
            .map(|sample| (*sample as i32 * volume as i32 / 100) as i16)
            .collect(),
    )
}

// Every event has its own raised-cosine attack/release and exponential decay.
// A finite harmonic sum (including the square-like pop) avoids discontinuities.
fn event(
    t: f64,
    start: f64,
    length: f64,
    from: f64,
    to: f64,
    decay: f64,
    partials: &[(f64, f64)],
) -> f64 {
    let time = t - start;
    if time <= 0.0 || time >= length {
        return 0.0;
    }
    let edge = |x: f64| 0.5 - 0.5 * (std::f64::consts::PI * x.min(1.0)).cos();
    let envelope = edge(time / 0.004) * edge((length - time) / 0.012) * (-time / decay).exp();
    let phase = TAU * (from * time + (to - from) * time * time / (2.0 * length));
    partials
        .iter()
        .map(|(ratio, gain)| (phase * ratio).sin() * gain)
        .sum::<f64>()
        * envelope
}

fn synthesize(index: usize) -> Vec<i16> {
    let lengths = [
        0.32, 0.48, 0.26, 0.56, 0.22, 0.38, 0.28, 0.34, 0.10, 0.14, 0.16, 0.08, 0.42, 0.30, 0.44,
        0.60, 0.70, 0.30, 0.52, 0.24,
    ];
    let length = lengths[index];
    let sine = &[(1.0, 1.0)][..];
    let bell = &[(1.0, 1.0), (2.76, 0.38), (4.07, 0.17)][..];
    let pluck = &[(1.0, 1.0), (2.0, 0.35), (3.0, 0.12)][..];
    let mut seed = 0x6d2b_79f5_u32;
    let mut low_noise = 0.0;
    let count = (length * SAMPLE_RATE as f64).round() as usize;
    let raw: Vec<f64> = (0..count)
        .map(|i| {
            let t = i as f64 / SAMPLE_RATE as f64;
            let tone = |start, len, from, to, decay, harmonics| {
                event(t, start, len, from, to, decay, harmonics)
            };
            match index {
                0 => tone(0.0, length, 740.0, 740.0, 0.095, sine),
                1 => tone(0.0, length, 1046.5, 1046.5, 0.15, bell),
                2 => tone(0.0, length, 2100.0, 1100.0, 0.07, bell),
                3 => {
                    tone(0.0, 0.34, 523.25, 523.25, 0.11, sine)
                        + tone(0.22, 0.34, 659.25, 659.25, 0.11, sine)
                }
                4 => {
                    tone(0.0, 0.07, 1600.0, 900.0, 0.012, pluck)
                        + tone(0.12, 0.10, 1400.0, 800.0, 0.014, pluck)
                }
                5 => [0.0, 0.13, 0.26]
                    .iter()
                    .map(|s| tone(*s, 0.12, 880.0, 880.0, 0.09, sine))
                    .sum(),
                6 => tone(0.0, length, 450.0, 2400.0, 0.22, pluck),
                7 => tone(0.0, length, 1800.0, 260.0, 0.15, sine),
                8 => tone(
                    0.0,
                    length,
                    360.0,
                    280.0,
                    0.028,
                    &[(1.0, 1.0), (3.0, 0.3), (5.0, 0.15)],
                ),
                9 => {
                    seed ^= seed << 13;
                    seed ^= seed >> 17;
                    seed ^= seed << 5;
                    low_noise =
                        low_noise * 0.85 + (seed as f64 / u32::MAX as f64 * 2.0 - 1.0) * 0.15;
                    tone(0.0, length, 220.0, 170.0, 0.025, pluck)
                        + low_noise
                            * (1.0 - (-t / 0.003).exp())
                            * (-t / 0.018).exp()
                            * (1.0 - t / length).powi(2)
                }
                10 => tone(0.0, length, 780.0, 120.0, 0.042, sine),
                11 => tone(0.0, length, 2800.0, 2500.0, 0.018, bell),
                12 => tone(
                    0.0,
                    length,
                    440.0,
                    440.0,
                    0.095,
                    &[(1.0, 1.0), (4.0, 0.25), (10.0, 0.04)],
                ),
                13 => tone(0.0, length, 160.0, 160.0, 0.15, sine),
                14 => {
                    tone(0.0, 0.15, 1180.0, 1180.0, 0.045, sine)
                        + 0.35 * tone(0.22, 0.22, 1180.0, 1180.0, 0.06, sine)
                }
                15 => [0.0, 0.19, 0.38]
                    .iter()
                    .enumerate()
                    .map(|(i, s)| {
                        0.45_f64.powi(i as i32) * tone(*s, 0.22, 587.33, 587.33, 0.06, pluck)
                    })
                    .sum(),
                16 => tone(
                    0.0,
                    length,
                    290.0,
                    290.0,
                    0.20,
                    &[(1.0, 1.0), (1.43, 0.55), (2.31, 0.25), (3.87, 0.12)],
                ),
                17 => {
                    tone(0.0, 0.13, 1318.5, 1318.5, 0.09, pluck)
                        + tone(0.13, 0.17, 1975.5, 1975.5, 0.08, pluck)
                }
                18 => [523.25, 659.25, 783.99]
                    .iter()
                    .map(|f| tone(0.0, length, *f, *f, 0.14, sine))
                    .sum(),
                19 => {
                    tone(0.0, 0.11, 600.0, 1700.0, 0.08, sine)
                        + tone(0.13, 0.11, 1700.0, 600.0, 0.08, sine)
                }
                _ => unreachable!(),
            }
        })
        .collect();
    let peak = raw.iter().fold(0.0_f64, |max, s| max.max(s.abs()));
    let rms = (raw.iter().map(|s| s * s).sum::<f64>() / count as f64).sqrt();
    let gain = (0.17 / rms).min(0.75 / peak);
    raw.into_iter()
        .map(|s| (s * gain * i16::MAX as f64).round() as i16)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn catalog_has_stable_unique_order() {
        let ids = [
            "soft-ping",
            "bright-bell",
            "crystal-drop",
            "gentle-chime",
            "double-tap",
            "triple-beep",
            "rising-spark",
            "falling-star",
            "digital-pop",
            "wooden-click",
            "bubble-pop",
            "glass-tick",
            "marimba-note",
            "low-pulse",
            "radar-blip",
            "echo-pluck",
            "tiny-gong",
            "arcade-coin",
            "success-chord",
            "alert-chirp",
        ];
        assert_eq!(catalog().iter().map(|s| s.id).collect::<Vec<_>>(), ids);
        assert_eq!(
            catalog()
                .iter()
                .map(|s| s.name)
                .collect::<HashSet<_>>()
                .len(),
            20
        );
        assert!(!is_known_id("missing"));
    }

    #[test]
    fn signals_are_distinct_bounded_cached_and_volume_scaled() {
        let mut signals = HashSet::new();
        for sound in catalog() {
            let full = samples(sound.id, 100).unwrap();
            assert!((3528..=35280).contains(&full.len()));
            assert!(full.iter().any(|s| s.abs() > 5000));
            assert!(full.iter().all(|s| s.unsigned_abs() <= 24576));
            assert_eq!(full[0], 0);
            assert!(full.last().unwrap().abs() < 5);
            assert!(signals.insert(full.clone()));
            assert!(samples(sound.id, 0).unwrap().iter().all(|s| *s == 0));
            assert_eq!(
                samples(sound.id, 50).unwrap(),
                full.iter()
                    .map(|s| (*s as i32 / 2) as i16)
                    .collect::<Vec<_>>()
            );
            assert!(std::ptr::eq(
                cached_samples(sound.id).unwrap(),
                cached_samples(sound.id).unwrap()
            ));
        }
    }

    /// Export without playing: cargo test export_action_sound_wavs -- --ignored.
    #[test]
    #[ignore]
    fn export_action_sound_wavs() {
        let directory =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/action-sound-review");
        std::fs::create_dir_all(&directory).unwrap();
        for sound in catalog() {
            let pcm = samples(sound.id, 100).unwrap();
            let size = (pcm.len() * 2) as u32;
            let mut wav = Vec::new();
            wav.extend_from_slice(b"RIFF");
            wav.extend_from_slice(&(36 + size).to_le_bytes());
            wav.extend_from_slice(b"WAVEfmt ");
            wav.extend_from_slice(&16u32.to_le_bytes());
            wav.extend_from_slice(&1u16.to_le_bytes());
            wav.extend_from_slice(&1u16.to_le_bytes());
            wav.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
            wav.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
            wav.extend_from_slice(&2u16.to_le_bytes());
            wav.extend_from_slice(&16u16.to_le_bytes());
            wav.extend_from_slice(b"data");
            wav.extend_from_slice(&size.to_le_bytes());
            for sample in pcm {
                wav.extend_from_slice(&sample.to_le_bytes());
            }
            std::fs::write(directory.join(format!("{}.wav", sound.id)), wav).unwrap();
        }
    }
}
