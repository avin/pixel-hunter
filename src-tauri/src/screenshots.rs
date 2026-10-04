//! Screenshot files of pixels: `<root>/<profileId>/<pixelId>.png` and `<pixelId>.thumb.png`.

use std::fs::{self, File};
use std::io::{self, BufWriter};
use std::path::{Path, PathBuf};

use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{imageops, Rgb, RgbImage};
use uuid::Uuid;

use crate::error::{AppError, AppResult};

const THUMBNAIL_WIDTH: u32 = 320;
const MARKER_RADIUS: f32 = 8.0;
const MARKER_COLOR: Rgb<u8> = Rgb([255, 40, 40]);

pub struct Screenshots {
    root: PathBuf,
}

impl Screenshots {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn profile_dir(&self, profile_id: Uuid) -> PathBuf {
        self.root.join(profile_id.to_string())
    }

    pub fn image_path(&self, profile_id: Uuid, pixel_id: Uuid) -> PathBuf {
        self.profile_dir(profile_id).join(format!("{pixel_id}.png"))
    }

    pub fn thumbnail_path(&self, profile_id: Uuid, pixel_id: Uuid) -> PathBuf {
        self.profile_dir(profile_id).join(format!("{pixel_id}.thumb.png"))
    }

    /// Failures are logged: a leftover file must not block removing the pixel from the config.
    pub fn remove_pixel(&self, profile_id: Uuid, pixel_id: Uuid) {
        for path in [
            self.image_path(profile_id, pixel_id),
            self.thumbnail_path(profile_id, pixel_id),
        ] {
            log_failure(&path, fs::remove_file(&path));
        }
    }

    pub fn remove_profile(&self, profile_id: Uuid) {
        let dir = self.profile_dir(profile_id);
        log_failure(&dir, fs::remove_dir_all(&dir));
    }
}

pub fn save_png(path: &Path, image: &RgbImage) -> AppResult<()> {
    let write = || -> image::ImageResult<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let file = BufWriter::new(File::create(path)?);
        image.write_with_encoder(PngEncoder::new_with_quality(
            file,
            CompressionType::Fast,
            FilterType::Adaptive,
        ))
    };
    write().map_err(|error| {
        AppError::Capture(format!("Не удалось сохранить снимок {}: {error}", path.display()))
    })
}

pub fn thumbnail(image: &RgbImage) -> RgbImage {
    let width = image.width().min(THUMBNAIL_WIDTH);
    let height = (u64::from(image.height()) * u64::from(width) / u64::from(image.width())).max(1);
    imageops::thumbnail(image, width, height as u32)
}

/// Draws a ring with a cross on the thumbnail where point `(x, y)` of the full image lands.
pub fn draw_marker(thumbnail: &mut RgbImage, full_width: u32, x: u32, y: u32) {
    let scale = thumbnail.width() as f32 / full_width as f32;
    let center_x = (x as f32 + 0.5) * scale;
    let center_y = (y as f32 + 0.5) * scale;
    let reach = MARKER_RADIUS as i64 + 2;
    let (column, row) = (center_x.floor() as i64, center_y.floor() as i64);

    for dy in -reach..=reach {
        for dx in -reach..=reach {
            let (px, py) = (column + dx, row + dy);
            if px < 0 || py < 0 || px >= thumbnail.width() as i64 || py >= thumbnail.height() as i64 {
                continue;
            }
            let distance = ((px as f32 + 0.5 - center_x).powi(2)
                + (py as f32 + 0.5 - center_y).powi(2))
            .sqrt();
            let is_ring = (distance - MARKER_RADIUS).abs() <= 1.0;
            let is_cross = (dx == 0 || dy == 0) && distance <= MARKER_RADIUS;
            if is_ring || is_cross {
                thumbnail.put_pixel(px as u32, py as u32, MARKER_COLOR);
            }
        }
    }
}

fn log_failure(path: &Path, result: io::Result<()>) {
    match result {
        Err(error) if error.kind() != io::ErrorKind::NotFound => {
            eprintln!("Не удалось удалить {}: {error}", path.display());
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_pixel_files_and_tolerates_missing_ones() {
        let dir = tempfile::tempdir().unwrap();
        let screenshots = Screenshots::new(dir.path().to_path_buf());
        let profile_id = Uuid::new_v4();
        let (removed, kept) = (Uuid::new_v4(), Uuid::new_v4());
        fs::create_dir_all(screenshots.profile_dir(profile_id)).unwrap();
        for pixel_id in [removed, kept] {
            fs::write(screenshots.image_path(profile_id, pixel_id), b"png").unwrap();
        }
        fs::write(screenshots.thumbnail_path(profile_id, removed), b"png").unwrap();

        screenshots.remove_pixel(profile_id, removed);
        screenshots.remove_pixel(profile_id, Uuid::new_v4());

        assert!(!screenshots.image_path(profile_id, removed).exists());
        assert!(!screenshots.thumbnail_path(profile_id, removed).exists());
        assert!(screenshots.image_path(profile_id, kept).exists());
    }

    #[test]
    fn thumbnail_is_at_most_320_wide_and_keeps_aspect_ratio() {
        assert_eq!(thumbnail(&RgbImage::new(3440, 1440)).dimensions(), (320, 133));
        assert_eq!(thumbnail(&RgbImage::new(100, 50)).dimensions(), (100, 50));
    }

    #[test]
    fn marks_the_scaled_point_and_stays_inside_the_image() {
        let mut image = RgbImage::new(320, 180);
        draw_marker(&mut image, 1920, 960, 540);
        assert_eq!(*image.get_pixel(160, 90), MARKER_COLOR);
        assert_eq!(*image.get_pixel(0, 0), Rgb([0, 0, 0]));

        let mut corner = RgbImage::new(4, 2);
        draw_marker(&mut corner, 4, 3, 1);
        assert_eq!(*corner.get_pixel(3, 1), MARKER_COLOR);
    }

    #[test]
    fn removes_profile_directory() {
        let dir = tempfile::tempdir().unwrap();
        let screenshots = Screenshots::new(dir.path().to_path_buf());
        let profile_id = Uuid::new_v4();
        let pixel_id = Uuid::new_v4();
        fs::create_dir_all(screenshots.profile_dir(profile_id)).unwrap();
        fs::write(screenshots.image_path(profile_id, pixel_id), b"png").unwrap();

        screenshots.remove_profile(profile_id);
        screenshots.remove_profile(Uuid::new_v4());

        assert!(!screenshots.profile_dir(profile_id).exists());
    }
}
