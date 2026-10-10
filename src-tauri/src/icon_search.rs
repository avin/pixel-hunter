//! Color template matching and decoded-template availability cache.
use image::RgbImage;
use opencv::{
    core::{self, Mat},
    imgproc,
    prelude::*,
};
use std::{collections::HashMap, fs, path::Path, time::SystemTime};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub enum IconResult {
    Similarity(f64),
    Unavailable(String),
}

impl IconResult {
    pub fn passes(&self, icon: &crate::config::Icon) -> bool {
        match self {
            Self::Similarity(score) if score.is_finite() => {
                let found = *score >= f64::from(icon.similarity_percent);
                match icon.condition {
                    crate::config::IconCondition::Present => found,
                    crate::config::IconCondition::Absent => !found,
                }
            }
            _ => false,
        }
    }
}

pub fn similarity(error: f64) -> Result<f64, String> {
    if !error.is_finite() {
        return Err("Поиск вернул неконечную оценку сходства".into());
    }
    Ok(100.0 * (1.0 - error.clamp(0.0, 1.0)))
}

fn matrix(image: &RgbImage) -> Result<Mat, String> {
    if image.width() == 0 || image.height() == 0 {
        return Err("Пустое изображение".into());
    }
    let bytes = Mat::from_slice(image.as_raw()).map_err(|e| e.to_string())?;
    bytes
        .reshape(3, image.height() as i32)
        .and_then(|matrix| matrix.try_clone())
        .map_err(|e| e.to_string())
}

fn search(image: &RgbImage, template: &Mat) -> Result<f64, String> {
    if image.width() < template.cols() as u32 || image.height() < template.rows() as u32 {
        return Err("Образец больше области поиска".into());
    }
    let bytes = Mat::from_slice(image.as_raw()).map_err(|e| e.to_string())?;
    let image = bytes
        .reshape(3, image.height() as i32)
        .map_err(|e| e.to_string())?;
    let mut result = Mat::default();
    imgproc::match_template_def(&image, template, &mut result, imgproc::TM_SQDIFF_NORMED)
        .map_err(|e| e.to_string())?;
    // Validate all raw scores before reduction: minMaxLoc silently ignores NaN.
    if !core::check_range_def(&result).map_err(|e| e.to_string())? {
        return Err("Поиск вернул неконечную оценку сходства".into());
    }
    let mut minimum = f64::NAN;
    core::min_max_loc(
        &result,
        Some(&mut minimum),
        None,
        None,
        None,
        &core::no_array(),
    )
    .map_err(|e| e.to_string())?;
    similarity(minimum)
}

struct Cached {
    length: u64,
    modified: SystemTime,
    template: Result<Mat, String>,
}
#[derive(Default)]
pub struct TemplateCache {
    entries: HashMap<Uuid, Cached>,
}
impl TemplateCache {
    pub fn retain(&mut self, ids: &[Uuid]) {
        self.entries.retain(|id, _| ids.contains(id));
    }
    pub fn check(&mut self, id: Uuid, path: &Path, image: &RgbImage) -> IconResult {
        let checked = (|| {
            let metadata = fs::metadata(path).map_err(|e| {
                self.entries.remove(&id);
                format!("Образец недоступен: {e}")
            })?;
            let modified = metadata
                .modified()
                .map_err(|e| format!("Не удалось проверить образец: {e}"))?;
            let changed = self
                .entries
                .get(&id)
                .is_none_or(|entry| entry.length != metadata.len() || entry.modified != modified);
            if changed {
                let template = image::open(path)
                    .map_err(|e| format!("Не удалось прочитать образец: {e}"))
                    .and_then(|image| {
                        let image = image.into_rgb8();
                        if image.as_raw().iter().all(|v| *v == 0) {
                            return Err("Полностью чёрный образец непригоден для поиска".into());
                        }
                        matrix(&image)
                    });
                self.entries.insert(
                    id,
                    Cached {
                        length: metadata.len(),
                        modified,
                        template,
                    },
                );
            }
            let entry = &self.entries[&id];
            search(image, entry.template.as_ref().map_err(Clone::clone)?)
        })();
        match checked {
            Ok(score) => IconResult::Similarity(score),
            Err(error) => IconResult::Unavailable(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn image(width: u32, height: u32, color: [u8; 3]) -> RgbImage {
        RgbImage::from_pixel(width, height, image::Rgb(color))
    }
    #[test]
    fn finds_moved_border_and_multiple_color_matches() {
        let template = image(2, 2, [220, 90, 30]);
        for positions in [vec![(1, 1)], vec![(4, 3)], vec![(0, 0), (4, 3)]] {
            let mut roi = image(6, 5, [10, 20, 30]);
            for (x, y) in positions {
                image::imageops::replace(&mut roi, &template, x, y);
            }
            assert!(search(&roi, &matrix(&template).unwrap()).unwrap() > 99.99);
        }
        assert!(search(&image(6, 5, [30, 90, 220]), &matrix(&template).unwrap()).unwrap() < 90.0);
    }
    #[test]
    fn inclusive_threshold_and_unavailable_never_pass_absence() {
        let mut icon = crate::config::Icon {
            id: Uuid::new_v4(),
            search_rect: crate::config::PhysicalRect {
                x: 0,
                y: 0,
                width: 2,
                height: 2,
            },
            template_rect: crate::config::PhysicalRect {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            },
            condition: crate::config::IconCondition::Present,
            similarity_percent: 90,
        };
        assert!(IconResult::Similarity(90.0).passes(&icon));
        icon.condition = crate::config::IconCondition::Absent;
        assert!(!IconResult::Similarity(90.0).passes(&icon));
        assert!(IconResult::Similarity(89.0).passes(&icon));
        assert!(!IconResult::Unavailable("missing".into()).passes(&icon));
        assert!(!IconResult::Similarity(f64::NAN).passes(&icon));
        assert!(similarity(f64::INFINITY).is_err());
    }
    #[test]
    fn cached_file_removal_corruption_and_recovery() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("template.png");
        let id = Uuid::new_v4();
        let roi = image(3, 3, [100, 50, 20]);
        let mut cache = TemplateCache::default();
        image(1, 1, [100, 50, 20]).save(&path).unwrap();
        assert!(matches!(
            cache.check(id, &path, &roi),
            IconResult::Similarity(_)
        ));
        fs::remove_file(&path).unwrap();
        assert!(matches!(
            cache.check(id, &path, &roi),
            IconResult::Unavailable(_)
        ));
        fs::write(&path, b"broken PNG").unwrap();
        assert!(matches!(
            cache.check(id, &path, &roi),
            IconResult::Unavailable(_)
        ));
        image(1, 1, [100, 50, 20]).save(&path).unwrap();
        assert!(matches!(
            cache.check(id, &path, &roi),
            IconResult::Similarity(_)
        ));
        assert!(matches!(
            cache.check(id, &path, &image(0, 0, [0, 0, 0])),
            IconResult::Unavailable(_)
        ));
    }
}
