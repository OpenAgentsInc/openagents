//! Renders the engine's flat sample pack: a red line cube and an unlit floor,
//! with a second cube far away in the fog. Usage: line_surface_capture OUTPUT.png
//!
//! Prints the brightest line pixel near and far so the fog fade is visible
//! in the terminal as well as in the image.
use glam::{Mat4, Vec3};
use verse::imported::{Renderer, flat, lighting::Lighting};
use verse_engine::presentation::Instance;

fn instance(model: &str, transform: Mat4) -> Instance {
    Instance {
        mount: None,
        actor: None,
        model: model.into(),
        transform,
        animation: 0.into(),
        time: 0.,
        animation_epoch: None,
        emission: Vec3::ZERO,
    }
}

fn main() -> Result<(), String> {
    let output = std::env::args()
        .nth(1)
        .ok_or("Expected an output PNG path")?;
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let pack = flat::sample_pack(dir.path())?;
    let (width, height) = (1280u32, 720u32);
    let atlas = verse::ui::Atlas::new(1.);
    let floor = [instance("flat/floor", Mat4::IDENTITY)];
    let mut renderer = Renderer::new(pack, dir.path(), width, height, &atlas, &floor)?;
    let eye = Vec3::new(0., 1.6, -6.);
    let view = verse::render::View {
        view_proj: Mat4::perspective_rh(60f32.to_radians(), 16. / 9., 0.1, 200.)
            * Mat4::look_at_rh(eye, Vec3::new(0., 1., 10.), Vec3::Y),
        eye,
    };
    let lighting = Lighting {
        ambient: Vec3::ZERO,
        density: 0.06,
        shadowed: 0,
        fog: Vec3::ZERO,
        ..Default::default()
    };
    let cubes = [
        instance(
            "flat/cube",
            Mat4::from_translation(Vec3::new(-2.5, 0., 0.)) * Mat4::from_scale(Vec3::splat(2.)),
        ),
        instance(
            "flat/cube",
            Mat4::from_translation(Vec3::new(1.5, 0., 60.)) * Mat4::from_scale(Vec3::splat(2.)),
        ),
    ];
    let pixels = renderer.draw(view, &cubes, &verse::ui::UiBatch::default(), &lighting)?;
    let brightest = |x0: u32, x1: u32| {
        (0..height)
            .flat_map(|y| (x0..x1).map(move |x| ((y * width + x) * 4) as usize))
            .map(|i| pixels[i])
            .max()
            .unwrap_or(0)
    };
    println!(
        "near line red {} far line red {}",
        brightest(0, width / 2),
        brightest(width / 2, width)
    );
    let file = std::fs::File::create(&output).map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .map_err(|e| e.to_string())?
        .write_image_data(&pixels)
        .map_err(|e| e.to_string())?;
    println!("wrote {output}");
    Ok(())
}
