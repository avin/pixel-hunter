#![allow(dead_code)]
// Diagnostic replay of actual DXGI frames, using the application's matcher unchanged.
#[path = "../../../../src-tauri/src/icon_search.rs"]
mod icon_search;

mod config {
    use serde::Deserialize;

    #[derive(Clone, Copy, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub enum IconCondition {
        Present,
        Absent,
    }

    #[derive(Clone, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Icon {
        pub condition: IconCondition,
        pub similarity_percent: u8,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    if std::env::args().any(|arg| arg == "--compare-opencv") {
        use opencv::{core::{self, Mat}, imgproc, prelude::*};
        let template = image::open(fixtures.join("template.png"))?.into_rgb8();
        let bytes = Mat::from_slice(template.as_raw())?;
        let template = bytes.reshape(3, template.height() as i32)?;
        for name in ["present-live-region.png", "absent-live-region.png", "empty-live-patch.png"] {
            let image = image::open(fixtures.join(name))?.into_rgb8();
            let bytes = Mat::from_slice(image.as_raw())?;
            let image = bytes.reshape(3, image.height() as i32)?;
            for (label, method) in [("SQDIFF_NORMED",imgproc::TM_SQDIFF_NORMED),("CCORR_NORMED",imgproc::TM_CCORR_NORMED),("CCOEFF_NORMED",imgproc::TM_CCOEFF_NORMED)] {
                let mut scores = Mat::default();
                imgproc::match_template_def(&image,&template,&mut scores,method)?;
                let (mut minimum,mut maximum) = (0.0,0.0);
                core::min_max_loc(&scores,Some(&mut minimum),Some(&mut maximum),None,None,&core::no_array())?;
                println!("{name} {label}: {}%",100.0 * if method == imgproc::TM_SQDIFF_NORMED {1.0-minimum} else {maximum});
            }
        }
        return Ok(());
    }
    let json = std::fs::read_to_string(fixtures.join("icon.json"))?;
    let present: config::Icon = serde_json::from_str(json.trim_start_matches('\u{feff}'))?;
    let mut absent = present.clone();
    absent.condition = config::IconCondition::Absent;
    let id = uuid::Uuid::new_v4();
    let mut cache = icon_search::TemplateCache::default();
    let mut results = Vec::new();
    for name in ["present-live-region.png", "absent-live-region.png", "empty-live-patch.png"] {
        let image = image::open(fixtures.join(name))?.into_rgb8();
        let result = cache.check(id, &fixtures.join("template.png"), &image);
        println!("{name}: {result:?}; present={}, absent={} (threshold {}%)",
            result.passes(&present), result.passes(&absent), present.similarity_percent);
        results.push(result);
    }
    assert!(results[0].passes(&present), "The actual template must be found");
    assert!(!results[1].passes(&present), "False positive: original template is absent after switching the real Total Commander tab");
    assert!(!results[2].passes(&present), "False positive: an empty real panel patch cannot contain the template");
    Ok(())
}
