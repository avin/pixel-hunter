//! Screenshot files of pixels: `<root>/<profileId>/<pixelId>.png` and `<pixelId>.thumb.png`.

use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{imageops, Rgb, RgbImage};
use uuid::Uuid;

use crate::config::PhysicalRect;
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
        self.profile_dir(profile_id)
            .join(format!("{pixel_id}.thumb.png"))
    }

    pub fn template_path(&self, profile_id: Uuid, icon_id: Uuid) -> PathBuf {
        self.profile_dir(profile_id)
            .join(format!("{icon_id}.template.png"))
    }

    pub fn remove_icon(&self, profile_id: Uuid, icon_id: Uuid) {
        self.remove_pixel(profile_id, icon_id);
        let path = self.template_path(profile_id, icon_id);
        log_failure(&path, fs::remove_file(&path));
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
    write().map_err(|error| save_error(path, error))
}

/// Full-monitor frames skip filter search and deflate: each row uses filter None,
/// and IDAT holds zlib stored blocks. Thumbnails stay on [`save_png`].
pub fn save_stored_png(path: &Path, image: &RgbImage) -> AppResult<()> {
    let write = || -> io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let mut file = BufWriter::new(File::create(path)?);
        write_stored_png(&mut file, image)?;
        file.flush()
    };
    write().map_err(|error| save_error(path, error))
}

fn save_error(path: &Path, error: impl std::fmt::Display) -> AppError {
    AppError::Capture(format!(
        "Не удалось сохранить снимок {}: {error}",
        path.display()
    ))
}

fn write_stored_png(out: &mut impl io::Write, image: &RgbImage) -> io::Result<()> {
    out.write_all(b"\x89PNG\r\n\x1a\n")?;
    write_chunk(out, b"IHDR", &ihdr(image))?;
    write_chunk(out, b"IDAT", &zlib_stored(&filtered_rgb(image)))?;
    write_chunk(out, b"IEND", &[])
}

fn ihdr(image: &RgbImage) -> [u8; 13] {
    let mut data = [0u8; 13];
    data[0..4].copy_from_slice(&image.width().to_be_bytes());
    data[4..8].copy_from_slice(&image.height().to_be_bytes());
    data[8] = 8; // bit depth
    data[9] = 2; // truecolor RGB
    data
}

/// Filter None on every row: a zero byte, then raw RGB.
fn filtered_rgb(image: &RgbImage) -> Vec<u8> {
    let width = image.width() as usize;
    let row_bytes = 1 + width * 3;
    let raw = image.as_raw();
    let mut filtered = vec![0u8; image.height() as usize * row_bytes];
    for (y, row) in filtered.chunks_mut(row_bytes).enumerate() {
        let start = y * width * 3;
        row[1..].copy_from_slice(&raw[start..start + width * 3]);
    }
    filtered
}

const STORED_BLOCK: usize = 65_535;

fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut zlib = Vec::with_capacity(data.len() + data.len() / STORED_BLOCK * 5 + 6);
    zlib.extend_from_slice(&[0x78, 0x01]); // deflate, no preset dictionary, fastest
    let mut rest = data;
    loop {
        let len = rest.len().min(STORED_BLOCK);
        let last = rest.len() <= STORED_BLOCK;
        zlib.push(if last { 0x01 } else { 0x00 }); // BFINAL + BTYPE stored, byte-aligned
        zlib.extend_from_slice(&(len as u16).to_le_bytes());
        zlib.extend_from_slice(&(!(len as u16)).to_le_bytes());
        zlib.extend_from_slice(&rest[..len]);
        if last {
            break;
        }
        rest = &rest[len..];
    }
    zlib.extend_from_slice(&adler32(data).to_be_bytes());
    zlib
}

fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65_521;
    let mut s1 = 1u32;
    let mut s2 = 0u32;
    for chunk in data.chunks(5552) {
        for &byte in chunk {
            s1 += u32::from(byte);
            s2 += s1;
        }
        s1 %= MOD;
        s2 %= MOD;
    }
    (s2 << 16) | s1
}

fn write_chunk(out: &mut impl io::Write, kind: &[u8; 4], data: &[u8]) -> io::Result<()> {
    out.write_all(&(data.len() as u32).to_be_bytes())?;
    out.write_all(kind)?;
    out.write_all(data)?;
    let mut crc_data = Vec::with_capacity(4 + data.len());
    crc_data.extend_from_slice(kind);
    crc_data.extend_from_slice(data);
    out.write_all(&crc32(&crc_data).to_be_bytes())
}

const CRC_TABLE: [u32; 256] = crc_table();

const fn crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut index = 0;
    while index < 256 {
        let mut crc = index as u32;
        let mut bit = 0;
        while bit < 8 {
            if crc & 1 == 1 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
            bit += 1;
        }
        table[index] = crc;
        index += 1;
    }
    table
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in data {
        let index = ((crc ^ u32::from(byte)) & 0xff) as usize;
        crc = CRC_TABLE[index] ^ (crc >> 8);
    }
    !crc
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
            if px < 0 || py < 0 || px >= thumbnail.width() as i64 || py >= thumbnail.height() as i64
            {
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

pub fn draw_regions(
    thumbnail: &mut RgbImage,
    full_width: u32,
    search: PhysicalRect,
    template: PhysicalRect,
) {
    let scale = thumbnail.width() as f64 / full_width as f64;
    for (rect, color) in [
        (search, Rgb([40, 150, 255])),
        (template, Rgb([255, 180, 40])),
    ] {
        let left = (f64::from(rect.x) * scale).floor() as u32;
        let top = (f64::from(rect.y) * scale).floor() as u32;
        let right = ((f64::from(rect.x) + f64::from(rect.width)) * scale).ceil() as u32;
        let bottom = ((f64::from(rect.y) + f64::from(rect.height)) * scale).ceil() as u32;
        for y in top..bottom.min(thumbnail.height()) {
            for x in left..right.min(thumbnail.width()) {
                if x == left || x + 1 == right || y == top || y + 1 == bottom {
                    thumbnail.put_pixel(x, y, color);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_thumbnail_has_distinct_frames_and_cleanup_removes_all_three_files() {
        let dir = tempfile::tempdir().unwrap();
        let screenshots = Screenshots::new(dir.path().to_path_buf());
        let (profile, id) = (Uuid::new_v4(), Uuid::new_v4());
        let mut image = RgbImage::new(20, 10);
        draw_regions(
            &mut image,
            20,
            PhysicalRect {
                x: 0,
                y: 0,
                width: 20,
                height: 10,
            },
            PhysicalRect {
                x: 5,
                y: 3,
                width: 3,
                height: 3,
            },
        );
        assert_eq!(image.get_pixel(19, 9).0, [40, 150, 255]);
        assert_eq!(image.get_pixel(5, 3).0, [255, 180, 40]);
        assert_eq!(image.get_pixel(6, 4).0, [0, 0, 0]);
        for path in [
            screenshots.image_path(profile, id),
            screenshots.thumbnail_path(profile, id),
            screenshots.template_path(profile, id),
        ] {
            save_png(&path, &image).unwrap();
        }
        screenshots.remove_icon(profile, id);
        screenshots.remove_icon(profile, id);
        assert_eq!(
            fs::read_dir(screenshots.profile_dir(profile))
                .unwrap()
                .count(),
            0
        );
    }

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
        assert_eq!(
            thumbnail(&RgbImage::new(3440, 1440)).dimensions(),
            (320, 133)
        );
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
    fn stores_a_full_screenshot_without_deflate_compression() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shot.png");
        let mut image = RgbImage::new(200, 120);
        image.put_pixel(0, 0, Rgb([10, 20, 30]));
        image.put_pixel(50, 60, Rgb([1, 2, 3]));
        image.put_pixel(199, 119, Rgb([200, 210, 220]));

        save_stored_png(&path, &image).unwrap();

        let decoded = image::open(&path).unwrap().into_rgb8();
        assert_eq!(decoded, image);

        let png = fs::read(&path).unwrap();
        let inflated = decode_stored_idat(&png);
        assert!(
            inflated.blocks > 1,
            "a full frame spans more than one stored block"
        );
        assert_eq!(inflated.data, filtered_rgb(&image));
    }

    struct StoredIdat {
        blocks: usize,
        data: Vec<u8>,
    }

    fn decode_stored_idat(png: &[u8]) -> StoredIdat {
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let mut offset = 8;
        let mut idat = Vec::new();
        while offset + 8 <= png.len() {
            let len = u32::from_be_bytes(png[offset..offset + 4].try_into().unwrap()) as usize;
            let kind = &png[offset + 4..offset + 8];
            let data = &png[offset + 8..offset + 8 + len];
            if kind == b"IDAT" {
                idat.extend_from_slice(data);
            }
            offset += 12 + len;
        }

        assert_eq!(&idat[..2], &[0x78, 0x01]);
        let mut cursor = 2;
        let mut data = Vec::new();
        let mut blocks = 0;
        loop {
            let header = idat[cursor];
            let btype = (header >> 1) & 0b11;
            assert_eq!(btype, 0, "IDAT block must be stored, not compressed");
            cursor += 1;
            let len = u16::from_le_bytes(idat[cursor..cursor + 2].try_into().unwrap()) as usize;
            let nlen = u16::from_le_bytes(idat[cursor + 2..cursor + 4].try_into().unwrap());
            assert_eq!(nlen, !(len as u16));
            cursor += 4;
            data.extend_from_slice(&idat[cursor..cursor + len]);
            cursor += len;
            blocks += 1;
            if header & 1 == 1 {
                break;
            }
        }
        StoredIdat { blocks, data }
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

#[cfg(test)]
mod capture_timing {
    use super::*;
    use crate::screen::Frame;
    use std::time::Instant;

    /// Synthetic CPU/file pipeline only: excludes DXGI, IPC, image decode and window show.
    #[test]
    #[ignore]
    fn capture_pipeline_timing() {
        let dir = tempfile::tempdir().unwrap();
        for (width, height) in [(1920, 1080), (2560, 1440)] {
            let frame = Frame {
                width,
                height,
                bgra: (0..width as usize * height as usize * 4)
                    .map(|i| ((i * 37 + i / 97) % 256) as u8)
                    .collect(),
            };
            for sample in 0..3 {
                let start = Instant::now();
                let image = frame.to_rgb_image();
                let converted = Instant::now();
                save_stored_png(&dir.path().join("frame.png"), &image).unwrap();
                let saved = Instant::now();
                println!("{width}x{height} sample={sample} rgb_ms={:.3} stored_png_ms={:.3} cpu_file_total_ms={:.3}",
                    (converted-start).as_secs_f64()*1000., (saved-converted).as_secs_f64()*1000., (saved-start).as_secs_f64()*1000.);
                let start = Instant::now();
                let pixels = frame.picker_pixels();
                let converted = Instant::now();
                let stored: std::sync::Arc<[u8]> = pixels.into();
                let response = stored.to_vec();
                let ready = Instant::now();
                std::hint::black_box(response);
                println!("{width}x{height} sample={sample} raw_rgba_ms={:.3} storage_and_response_copy_ms={:.3} raw_pre_ipc_total_ms={:.3}",
                    (converted-start).as_secs_f64()*1000., (ready-converted).as_secs_f64()*1000., (ready-start).as_secs_f64()*1000.);
            }
        }
    }
}
