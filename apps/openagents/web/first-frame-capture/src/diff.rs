//! `first-frame-diff A.png B.png [HEATMAP.png]`: how far two frames of the
//! same size differ, pixel by pixel, as JSON on standard output. A pixel's
//! difference is its largest channel difference, 0 to 255. With a third
//! path, writes a heatmap: black where the frames agree, brighter where
//! they differ.

use anyhow::{Context, Result, bail, ensure};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        bail!("usage: first-frame-diff A.png B.png [HEATMAP.png]");
    }
    let open = |p: &str| -> Result<image::RgbaImage> {
        Ok(image::open(p)
            .with_context(|| format!("reading {p}"))?
            .to_rgba8())
    };
    let (a, b) = (open(&args[0])?, open(&args[1])?);
    ensure!(
        a.dimensions() == b.dimensions(),
        "the frames differ in size"
    );
    let mut heat = image::GrayImage::new(a.width(), a.height());
    let mut over = [0u64; 5];
    let limits = [0u8, 2, 8, 32, 128];
    let (mut sum, mut max, mut squares) = (0u64, 0u8, 0f64);
    for (x, y, pa) in a.enumerate_pixels() {
        let pb = b.get_pixel(x, y);
        let d = (0..3).map(|c| pa[c].abs_diff(pb[c])).max().unwrap_or(0);
        for (n, limit) in over.iter_mut().zip(limits) {
            if d > limit {
                *n += 1;
            }
        }
        for c in 0..3 {
            squares += (pa[c] as f64 - pb[c] as f64).powi(2);
        }
        sum += d as u64;
        max = max.max(d);
        heat.put_pixel(x, y, image::Luma([d.saturating_mul(4)]));
    }
    let pixels = (a.width() * a.height()) as f64;
    let mse = squares / (pixels * 3.0);
    let psnr = if mse == 0.0 {
        f64::INFINITY
    } else {
        10.0 * (255.0f64.powi(2) / mse).log10()
    };
    let share = |n: u64| (n as f64 / pixels * 1e4).round() / 1e2;
    println!(
        "{}",
        serde_json::json!({
            "width": a.width(),
            "height": a.height(),
            "identical_percent": 100.0 - share(over[0]),
            "over_2_percent": share(over[1]),
            "over_8_percent": share(over[2]),
            "over_32_percent": share(over[3]),
            "over_128_percent": share(over[4]),
            "mean": (sum as f64 / pixels * 1e3).round() / 1e3,
            "max": max,
            "psnr_db": if psnr.is_finite() { serde_json::json!((psnr * 100.0).round() / 100.0) } else { serde_json::json!("identical") },
        })
    );
    if let Some(path) = args.get(2) {
        heat.save(path)?;
    }
    Ok(())
}
