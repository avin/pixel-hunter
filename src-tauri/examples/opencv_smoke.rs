use opencv::{
    core::{self, Mat, Vec3b},
    imgproc,
};

fn main() -> opencv::Result<()> {
    let background = Vec3b::from([10, 20, 30]);
    let selected = Vec3b::from([90, 180, 220]);
    let image = Mat::from_slice_2d(&[
        [background, background, background],
        [background, selected, background],
    ])?;
    let template = Mat::from_slice_2d(&[[selected]])?;
    let mut result = Mat::default();
    imgproc::match_template_def(&image, &template, &mut result, imgproc::TM_SQDIFF_NORMED)?;
    let mut minimum = f64::NAN;
    let mut position = core::Point::default();
    core::min_max_loc(
        &result,
        Some(&mut minimum),
        None,
        Some(&mut position),
        None,
        &core::no_array(),
    )?;
    assert!(minimum.is_finite() && minimum.abs() < 1e-6);
    assert_eq!(position, core::Point::new(1, 1));
    println!(
        "OpenCV {}: color matchTemplate passed at (1, 1)",
        core::get_version_string()?
    );
    Ok(())
}
