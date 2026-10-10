//! Primary monitor capture through DXGI Desktop Duplication.

use image::RgbImage;
use windows::core::Interface;
use windows::Win32::Foundation::{HMODULE, POINT};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_UNKNOWN;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D, D3D11_BOX,
    D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAPPED_SUBRESOURCE,
    D3D11_MAP_READ, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
    D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIAdapter1, IDXGIFactory1, IDXGIOutput1, IDXGIOutputDuplication,
    DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_WAIT_TIMEOUT, DXGI_OUTDUPL_FRAME_INFO,
};
use windows::Win32::Graphics::Gdi::{MonitorFromPoint, MONITOR_DEFAULTTOPRIMARY};

use crate::config::{PhysicalRect, Rgb};
use crate::error::{AppError, AppResult};
use std::collections::HashMap;
use uuid::Uuid;

const FRAME_TIMEOUT_MS: u32 = 100;
/// `AcquireNextFrame` calls per request; covers the wait for the first desktop image.
const MAX_ACQUIRE_ATTEMPTS: u32 = 20;
const MAX_RECREATE_ATTEMPTS: u32 = 3;

/// Full BGRA image of the primary monitor in physical pixels.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

impl Frame {
    pub fn rgb_at(&self, x: u32, y: u32) -> Option<Rgb> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let offset = (y as usize * self.width as usize + x as usize) * 4;
        Some(bgra_to_rgb(&self.bgra[offset..offset + 4]))
    }

    pub fn to_rgb_image(&self) -> RgbImage {
        let rgb = self
            .bgra
            .chunks_exact(4)
            .flat_map(|pixel| {
                let Rgb { r, g, b } = bgra_to_rgb(pixel);
                [r, g, b]
            })
            .collect();
        RgbImage::from_raw(self.width, self.height, rgb).expect("frame buffer matches its size")
    }
}

fn bgra_to_rgb(pixel: &[u8]) -> Rgb {
    Rgb {
        r: pixel[2],
        g: pixel[1],
        b: pixel[0],
    }
}

/// Source box of every point paired with the column of the N×1 staging texture it lands in.
fn point_copies(points: &[(u32, u32)]) -> impl Iterator<Item = (u32, D3D11_BOX)> + '_ {
    points.iter().enumerate().map(|(column, &(x, y))| {
        let source = D3D11_BOX {
            left: x,
            top: y,
            front: 0,
            right: x + 1,
            bottom: y + 1,
            back: 1,
        };
        (column as u32, source)
    })
}

pub struct PrimaryScreen {
    output: IDXGIOutput1,
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    duplication: Option<IDXGIOutputDuplication>,
    /// Copy of the last acquired desktop image; `AcquireNextFrame` only returns changes.
    last_frame: Option<ID3D11Texture2D>,
    points_staging: Option<(u32, ID3D11Texture2D)>,
    roi_staging: HashMap<Uuid, (u32, u32, ID3D11Texture2D)>,
}

impl PrimaryScreen {
    pub fn new() -> AppResult<Self> {
        unsafe {
            let (adapter, output) = primary_output()?;
            let mut device = None;
            let mut context = None;
            D3D11CreateDevice(
                &adapter,
                D3D_DRIVER_TYPE_UNKNOWN,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )
            .map_err(capture_error)?;
            let (Some(device), Some(context)) = (device, context) else {
                return Err(AppError::Capture("Direct3D не создал устройство".into()));
            };
            Ok(Self {
                output,
                device,
                context,
                duplication: None,
                last_frame: None,
                points_staging: None,
                roi_staging: HashMap::new(),
            })
        }
    }

    pub fn capture_frame(&mut self) -> AppResult<Frame> {
        let frame = self.current_frame()?;
        let desc = bgra_frame_desc(&frame)?;
        let staging = self.create_staging(desc.Width, desc.Height)?;
        unsafe { self.context.CopyResource(&staging, &frame) };
        let bgra = self.read_staging(&staging, desc.Width, desc.Height)?;
        Ok(Frame {
            width: desc.Width,
            height: desc.Height,
            bgra,
        })
    }

    pub fn read_pixels(&mut self, points: &[(u32, u32)]) -> AppResult<Vec<Rgb>> {
        if points.is_empty() {
            return Ok(Vec::new());
        }
        let frame = self.current_frame()?;
        self.read_points_from(&frame, points)
    }

    fn read_points_from(
        &mut self,
        frame: &ID3D11Texture2D,
        points: &[(u32, u32)],
    ) -> AppResult<Vec<Rgb>> {
        if points.is_empty() {
            return Ok(vec![]);
        }
        let desc = bgra_frame_desc(frame)?;
        if let Some(&(x, y)) = points
            .iter()
            .find(|&&(x, y)| x >= desc.Width || y >= desc.Height)
        {
            return Err(AppError::Capture(format!(
                "Точка ({x}, {y}) вне основного монитора {}×{}",
                desc.Width, desc.Height
            )));
        }

        let columns = points.len() as u32;
        let staging = self.points_staging(columns)?;
        for (column, source) in point_copies(points) {
            unsafe {
                self.context.CopySubresourceRegion(
                    &staging,
                    0,
                    column,
                    0,
                    0,
                    frame,
                    0,
                    Some(&source),
                );
            }
        }
        let row = self.read_staging(&staging, columns, 1)?;
        Ok(row.chunks_exact(4).map(bgra_to_rgb).collect())
    }

    /// All copies refer to the same acquired texture; no second capture between points and ROIs.
    pub fn read_visuals(
        &mut self,
        points: &[(u32, u32)],
        rectangles: &[(Uuid, PhysicalRect)],
    ) -> AppResult<(Vec<Rgb>, Vec<(Uuid, Result<RgbImage, String>)>)> {
        if rectangles.is_empty() {
            return self.read_pixels(points).map(|colors| (colors, vec![]));
        }
        let frame = self.current_frame()?;
        let desc = bgra_frame_desc(&frame)?;
        let colors = self.read_points_from(&frame, points)?;
        self.roi_staging
            .retain(|id, _| rectangles.iter().any(|(active, _)| active == id));
        let mut images = Vec::with_capacity(rectangles.len());
        for (id, rect) in rectangles {
            let image = (|| -> AppResult<RgbImage> {
                if !rect_fits(*rect, desc.Width, desc.Height) {
                    return Err(AppError::Capture(
                        "Область поиска вне текущего экрана".into(),
                    ));
                }
                let cached = self
                    .roi_staging
                    .get(id)
                    .filter(|(w, h, _)| *w == rect.width && *h == rect.height)
                    .map(|(_, _, t)| t.clone());
                let staging = match cached {
                    Some(texture) => texture,
                    None => {
                        let texture = self.create_staging(rect.width, rect.height)?;
                        self.roi_staging
                            .insert(*id, (rect.width, rect.height, texture.clone()));
                        texture
                    }
                };
                let source = D3D11_BOX {
                    left: rect.x,
                    top: rect.y,
                    right: rect.x + rect.width,
                    bottom: rect.y + rect.height,
                    front: 0,
                    back: 1,
                };
                unsafe {
                    self.context.CopySubresourceRegion(
                        &staging,
                        0,
                        0,
                        0,
                        0,
                        &frame,
                        0,
                        Some(&source),
                    );
                }
                let bgra = self.read_staging(&staging, rect.width, rect.height)?;
                let rgb = bgra
                    .chunks_exact(4)
                    .flat_map(|p| [p[2], p[1], p[0]])
                    .collect();
                RgbImage::from_raw(rect.width, rect.height, rgb)
                    .ok_or_else(|| AppError::Capture("Некорректный размер области".into()))
            })()
            .map_err(|e| e.to_string());
            images.push((*id, image));
        }
        Ok((colors, images))
    }

    fn current_frame(&mut self) -> AppResult<ID3D11Texture2D> {
        let mut recreations = 0;
        let mut last_error = None;
        for _ in 0..MAX_ACQUIRE_ATTEMPTS {
            let duplication = match &self.duplication {
                Some(duplication) => duplication.clone(),
                None => match unsafe { self.output.DuplicateOutput(&self.device) } {
                    Ok(duplication) => {
                        self.duplication = Some(duplication.clone());
                        duplication
                    }
                    Err(error) => {
                        recreations += 1;
                        last_error = Some(error);
                        if recreations >= MAX_RECREATE_ATTEMPTS {
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(50));
                        continue;
                    }
                },
            };

            match self.acquire(&duplication) {
                Ok(()) => {
                    if let Some(frame) = &self.last_frame {
                        return Ok(frame.clone());
                    }
                }
                Err(error) if error.code() == DXGI_ERROR_WAIT_TIMEOUT => {
                    if let Some(frame) = &self.last_frame {
                        return Ok(frame.clone());
                    }
                    last_error = Some(error);
                }
                Err(error) if error.code() == DXGI_ERROR_ACCESS_LOST => {
                    self.duplication = None;
                    self.last_frame = None;
                    recreations += 1;
                    last_error = Some(error);
                    if recreations >= MAX_RECREATE_ATTEMPTS {
                        break;
                    }
                }
                Err(error) => return Err(capture_error(error)),
            }
        }
        Err(match last_error {
            Some(error) => capture_error(error),
            None => AppError::Capture("Не удалось получить кадр экрана".into()),
        })
    }

    /// Waits for a frame only until the first image; after that an unchanged desktop means the
    /// stored image is current, and waiting would stall the 50 ms polling.
    fn acquire(&mut self, duplication: &IDXGIOutputDuplication) -> windows::core::Result<()> {
        let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut resource = None;
        let timeout = if self.last_frame.is_some() {
            0
        } else {
            FRAME_TIMEOUT_MS
        };
        unsafe { duplication.AcquireNextFrame(timeout, &mut info, &mut resource)? };
        // A frame without a present is a pointer-only update; its surface may hold no desktop image.
        let stored = if info.LastPresentTime == 0 {
            Ok(())
        } else {
            resource
                .ok_or_else(windows::core::Error::empty)
                .and_then(|resource| resource.cast::<ID3D11Texture2D>())
                .and_then(|texture| self.store_frame(&texture))
        };
        unsafe {
            let _ = duplication.ReleaseFrame();
        }
        stored
    }

    fn store_frame(&mut self, texture: &ID3D11Texture2D) -> windows::core::Result<()> {
        let source = texture_desc(texture);
        let reusable = self.last_frame.as_ref().filter(|frame| {
            let desc = texture_desc(frame);
            desc.Width == source.Width && desc.Height == source.Height
        });
        let target = match reusable {
            Some(frame) => frame.clone(),
            None => {
                let desc = D3D11_TEXTURE2D_DESC {
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: 0,
                    CPUAccessFlags: 0,
                    MiscFlags: 0,
                    ..source
                };
                let mut frame = None;
                unsafe { self.device.CreateTexture2D(&desc, None, Some(&mut frame))? };
                frame.ok_or_else(windows::core::Error::empty)?
            }
        };
        unsafe { self.context.CopyResource(&target, texture) };
        self.last_frame = Some(target);
        Ok(())
    }

    fn points_staging(&mut self, columns: u32) -> AppResult<ID3D11Texture2D> {
        if let Some((cached_columns, texture)) = &self.points_staging {
            if *cached_columns == columns {
                return Ok(texture.clone());
            }
        }
        let texture = self.create_staging(columns, 1)?;
        self.points_staging = Some((columns, texture.clone()));
        Ok(texture)
    }

    fn create_staging(&self, width: u32, height: u32) -> AppResult<ID3D11Texture2D> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
        };
        let mut texture = None;
        unsafe {
            self.device
                .CreateTexture2D(&desc, None, Some(&mut texture))
                .map_err(capture_error)?;
        }
        texture.ok_or_else(|| AppError::Capture("Direct3D не создал текстуру".into()))
    }

    fn read_staging(
        &self,
        staging: &ID3D11Texture2D,
        width: u32,
        height: u32,
    ) -> AppResult<Vec<u8>> {
        let row_bytes = width as usize * 4;
        let mut bgra = Vec::with_capacity(row_bytes * height as usize);
        unsafe {
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            self.context
                .Map(staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                .map_err(capture_error)?;
            for row in 0..height as usize {
                let start = (mapped.pData as *const u8).add(row * mapped.RowPitch as usize);
                bgra.extend_from_slice(std::slice::from_raw_parts(start, row_bytes));
            }
            self.context.Unmap(staging, 0);
        }
        Ok(bgra)
    }
}

fn rect_fits(rect: PhysicalRect, width: u32, height: u32) -> bool {
    rect.width > 0
        && rect.height > 0
        && rect
            .x
            .checked_add(rect.width)
            .is_some_and(|edge| edge <= width)
        && rect
            .y
            .checked_add(rect.height)
            .is_some_and(|edge| edge <= height)
}

fn primary_output() -> AppResult<(IDXGIAdapter1, IDXGIOutput1)> {
    unsafe {
        let primary = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        let factory: IDXGIFactory1 = CreateDXGIFactory1().map_err(capture_error)?;
        let mut adapter_index = 0;
        while let Ok(adapter) = factory.EnumAdapters1(adapter_index) {
            let mut output_index = 0;
            while let Ok(output) = adapter.EnumOutputs(output_index) {
                if output.GetDesc().is_ok_and(|desc| desc.Monitor == primary) {
                    let output = output.cast::<IDXGIOutput1>().map_err(capture_error)?;
                    return Ok((adapter, output));
                }
                output_index += 1;
            }
            adapter_index += 1;
        }
    }
    Err(AppError::Capture(
        "Основной монитор не найден среди выходов DXGI".into(),
    ))
}

fn texture_desc(texture: &ID3D11Texture2D) -> D3D11_TEXTURE2D_DESC {
    let mut desc = D3D11_TEXTURE2D_DESC::default();
    unsafe { texture.GetDesc(&mut desc) };
    desc
}

fn bgra_frame_desc(frame: &ID3D11Texture2D) -> AppResult<D3D11_TEXTURE2D_DESC> {
    let desc = texture_desc(frame);
    if desc.Format != DXGI_FORMAT_B8G8R8A8_UNORM {
        return Err(AppError::Capture(
            "Формат кадра экрана не поддерживается; выключите HDR на основном мониторе".into(),
        ));
    }
    Ok(desc)
}

fn capture_error(error: windows::core::Error) -> AppError {
    AppError::Capture(format!("Не удалось захватить экран: {}", error.message()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_rgb_from_bgra_frame() {
        let frame = Frame {
            width: 2,
            height: 2,
            bgra: vec![
                1, 2, 3, 255, 30, 30, 200, 255, //
                0, 0, 0, 255, 255, 128, 0, 255,
            ],
        };

        assert_eq!(
            frame.rgb_at(1, 0),
            Some(Rgb {
                r: 200,
                g: 30,
                b: 30
            })
        );
        assert_eq!(
            frame.rgb_at(1, 1),
            Some(Rgb {
                r: 0,
                g: 128,
                b: 255
            })
        );
        assert_eq!(frame.rgb_at(2, 0), None);
        assert_eq!(frame.rgb_at(0, 2), None);
        assert_eq!(
            frame.to_rgb_image().into_raw(),
            [3, 2, 1, 200, 30, 30, 0, 0, 0, 0, 128, 255]
        );
    }

    #[test]
    fn copies_each_point_into_its_own_column() {
        let copies: Vec<_> = point_copies(&[(812, 1040), (0, 0), (3839, 2159)])
            .map(|(column, source)| {
                (
                    column,
                    (source.left, source.top, source.right, source.bottom),
                )
            })
            .collect();

        assert_eq!(
            copies,
            [
                (0, (812, 1040, 813, 1041)),
                (1, (0, 0, 1, 1)),
                (2, (3839, 2159, 3840, 2160)),
            ]
        );
        assert!(point_copies(&[(5, 7)]).all(|(_, source)| source.front == 0 && source.back == 1));
    }

    #[test]
    fn roi_bounds_are_half_open_and_overflow_safe() {
        assert!(rect_fits(
            PhysicalRect {
                x: 2,
                y: 3,
                width: 8,
                height: 7
            },
            10,
            10
        ));
        for rect in [
            PhysicalRect {
                x: 0,
                y: 0,
                width: 0,
                height: 1,
            },
            PhysicalRect {
                x: 9,
                y: 0,
                width: 2,
                height: 1,
            },
            PhysicalRect {
                x: u32::MAX,
                y: 0,
                width: 2,
                height: 1,
            },
        ] {
            assert!(!rect_fits(rect, 10, 10));
        }
    }
    #[test]
    #[ignore]
    fn measures_visual_poll_on_windows() {
        use crate::icon_search::{IconResult, TemplateCache};
        use std::time::Instant;
        let mut screen = PrimaryScreen::new().unwrap();
        let frame = screen.capture_frame().unwrap();
        let rgb = frame.to_rgb_image();
        let dir = tempfile::tempdir().unwrap();
        let mut cache = TemplateCache::default();
        let rect = PhysicalRect {
            x: 0,
            y: 0,
            width: frame.width.min(1200),
            height: frame.height.min(800),
        };
        let ids: Vec<_> = (0..3).map(|_| Uuid::new_v4()).collect();
        let template = image::imageops::crop_imm(&rgb, 0, 0, 24, 24).to_image();
        for id in &ids {
            template.save(dir.path().join(format!("{id}.png"))).unwrap();
        }
        let rectangles: Vec<_> = ids.iter().map(|id| (*id, rect)).collect();
        let points = [(0, 0), (frame.width - 1, frame.height - 1)];
        let mut pixel_times = vec![];
        let mut pixel_periods = vec![];
        let mut previous_pixel = None;
        for _ in 0..100 {
            let start = Instant::now();
            if let Some(last) = previous_pixel.replace(start) {
                pixel_periods.push(start.duration_since(last).as_secs_f64() * 1000.0);
            }
            screen.read_pixels(&points).unwrap();
            let elapsed = start.elapsed();
            pixel_times.push(elapsed.as_secs_f64() * 1000.0);
            std::thread::sleep(std::time::Duration::from_millis(50).saturating_sub(elapsed));
        }
        pixel_times.sort_by(f64::total_cmp);
        pixel_periods.sort_by(f64::total_cmp);
        println!(
            "Pixels-only scheduled polling period p95 {:.3}ms",
            pixel_periods[93]
        );
        println!(
            "Pixels-only same compact path: 2 points n100 p95 {:.3}ms",
            pixel_times[94]
        );
        let mut elapsed = Vec::new();
        let mut readback = Vec::new();
        let mut matching = Vec::new();
        let mut periods = Vec::new();
        let mut previous = None;
        for _ in 0..100 {
            let start = Instant::now();
            if let Some(last) = previous.replace(start) {
                periods.push(start.duration_since(last).as_secs_f64() * 1000.0);
            }
            let (colors, images) = screen.read_visuals(&points, &rectangles).unwrap();
            let copied = start.elapsed();
            assert_eq!(colors.len(), 2);
            for (id, image) in images {
                let image = image.unwrap();
                let pixel = image.get_pixel(0, 0).0;
                assert_eq!(
                    colors[0],
                    Rgb {
                        r: pixel[0],
                        g: pixel[1],
                        b: pixel[2]
                    },
                    "point and ROI must read the same acquired texture"
                );
                let result = cache.check(id, &dir.path().join(format!("{id}.png")), &image);
                assert!(matches!(result, IconResult::Similarity(_)));
            }
            let total = start.elapsed();
            readback.push(copied.as_secs_f64() * 1000.0);
            matching.push((total - copied).as_secs_f64() * 1000.0);
            elapsed.push(total.as_secs_f64() * 1000.0);
            std::thread::sleep(std::time::Duration::from_millis(50).saturating_sub(total));
        }
        let p95 = |samples: &mut Vec<f64>| {
            samples.sort_by(f64::total_cmp);
            samples[94]
        };
        periods.sort_by(f64::total_cmp);
        println!("Visual polling actual start-to-start p95 {:.2} ms (50ms schedule, no overlapping jobs)",periods[93]);
        let misses = elapsed.iter().filter(|ms| **ms > 50.0).count();
        println!("Windows actual DXGI {}x{}, 2 points + 3 ROIs {}x{}, templates24x24, n100: readback p95 {:.2} ms, matching p95 {:.2} ms, total p95 {:.2} ms, deadline misses {}/100",frame.width,frame.height,rect.width,rect.height,p95(&mut readback),p95(&mut matching),p95(&mut elapsed),misses);
    }

    /// Saves the primary monitor to a PNG in the temp directory:
    /// `cargo test -- --ignored saves_primary_screen_png --nocapture`.
    #[test]
    #[ignore]
    fn saves_primary_screen_png() {
        let mut screen = PrimaryScreen::new().unwrap();
        let frame = screen.capture_frame().unwrap();
        let path = std::env::temp_dir().join("pixel-hunter-primary-screen.png");
        frame.to_rgb_image().save(&path).unwrap();
        println!(
            "{}×{} saved to {}",
            frame.width,
            frame.height,
            path.display()
        );

        // Corners rarely change between the two reads, unlike the middle of the screen.
        let points = [
            (0, 0),
            (frame.width - 1, 0),
            (frame.width - 1, frame.height - 1),
        ];
        let colors = screen.read_pixels(&points).unwrap();
        let expected: Vec<_> = points
            .iter()
            .map(|&(x, y)| frame.rgb_at(x, y).unwrap())
            .collect();
        assert_eq!(colors, expected);
    }
}
