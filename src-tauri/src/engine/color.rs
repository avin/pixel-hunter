use crate::config::{Pixel, PixelCondition, Rgb};

/// Every channel differs by at most `tolerance% × 255 / 100`, compared in integers.
fn matches(sample: Rgb, reference: Rgb, tolerance_percent: u8) -> bool {
    let limit = u32::from(tolerance_percent) * 255;
    [
        (sample.r, reference.r),
        (sample.g, reference.g),
        (sample.b, reference.b),
    ]
    .into_iter()
    .all(|(actual, expected)| u32::from(actual.abs_diff(expected)) * 100 <= limit)
}

pub fn pixel_passes(sample: Rgb, pixel: &Pixel) -> bool {
    let matched = matches(sample, pixel.color, pixel.tolerance_percent);
    match pixel.condition {
        PixelCondition::Equals => matched,
        PixelCondition::NotEquals => !matched,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    const REFERENCE: Rgb = Rgb { r: 200, g: 30, b: 30 };

    fn pixel(condition: PixelCondition, tolerance_percent: u8) -> Pixel {
        let mut pixel = Pixel::new(Uuid::new_v4(), 0, 0, REFERENCE);
        pixel.condition = condition;
        pixel.tolerance_percent = tolerance_percent;
        pixel
    }

    fn rgb(r: u8, g: u8, b: u8) -> Rgb {
        Rgb { r, g, b }
    }

    #[test]
    fn matches_within_tolerance() {
        assert!(pixel_passes(rgb(225, 10, 50), &pixel(PixelCondition::Equals, 10)));
    }

    #[test]
    fn one_channel_outside_tolerance_does_not_match() {
        assert!(!pixel_passes(rgb(200, 30, 57), &pixel(PixelCondition::Equals, 10)));
    }

    #[test]
    fn ten_percent_allows_a_difference_of_25_but_not_26() {
        let equals = pixel(PixelCondition::Equals, 10);

        assert!(pixel_passes(rgb(225, 5, 55), &equals));
        assert!(!pixel_passes(rgb(226, 30, 30), &equals));
        assert!(!pixel_passes(rgb(200, 4, 30), &equals));
    }

    #[test]
    fn zero_tolerance_requires_the_exact_color() {
        let equals = pixel(PixelCondition::Equals, 0);

        assert!(pixel_passes(REFERENCE, &equals));
        assert!(!pixel_passes(rgb(200, 31, 30), &equals));
    }

    #[test]
    fn full_tolerance_matches_any_color() {
        assert!(pixel_passes(rgb(0, 255, 255), &pixel(PixelCondition::Equals, 100)));
    }

    #[test]
    fn not_equals_passes_when_the_color_does_not_match() {
        let not_equals = pixel(PixelCondition::NotEquals, 10);

        assert!(pixel_passes(rgb(200, 30, 57), &not_equals));
        assert!(!pixel_passes(rgb(225, 10, 50), &not_equals));
    }
}
