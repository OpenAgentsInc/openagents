//! Native GPU inspection of every retained Standard character appearance.
use glam::{Mat4, Vec3};
use std::path::{Path, PathBuf};
use verse::{
    imported::{
        Instance, Renderer, chamber, characters,
        lighting::{Light, Lighting},
        original,
    },
    render::View,
    ui::UiBatch,
};
fn main() -> Result<(), String> {
    let output = PathBuf::from(std::env::args().nth(1).ok_or("Expected output.png")?);
    let dir = std::env::temp_dir().join(format!("verse-universal-gallery-{}", std::process::id()));
    let mut pack = original::generate(&dir)?;
    characters::install(
        &mut pack,
        &dir,
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/verse/characters/quaternius"),
        "male-ranger",
    )?;
    let atlas = original::atlas()?;
    let mut renderer = Renderer::new(pack, &dir, 1600, 560, &atlas, &[])?;
    let eye = Vec3::new(0., 2., -8.);
    let view = View {
        eye,
        view_proj: Mat4::perspective_rh(30_f32.to_radians(), 1600. / 560., 0.01, 50.)
            * Mat4::look_at_rh(eye, Vec3::Y * 0.9, Vec3::Y),
    };
    let mut instances: Vec<_> = characters::APPEARANCES
        .iter()
        .enumerate()
        .map(|(i, name)| Instance {
            actor: None,
            model: format!("universal-{name}"),
            transform: Mat4::from_translation(Vec3::new((2.5 - i as f32) * 1.9, 0., 0.))
                * chamber::basis(),
            animation: 0,
            time: 0.4,
            emission: Vec3::ONE,
        })
        .collect();
    let mut ui = UiBatch::default();
    for (i, name) in characters::APPEARANCES.iter().enumerate() {
        ui.text(&atlas, 65. + i as f32 * 255., 515., name, [1., 1., 1., 1.]);
    }
    let lighting = Lighting {
        ambient: Vec3::splat(0.3),
        fog: Vec3::splat(0.012),
        density: 0.,
        lights: vec![Light {
            position: Vec3::new(0., 4., -4.),
            color: Vec3::new(1., 0.85, 0.7),
            intensity: 45.,
            range: 20.,
        }],
        shadowed: 0,
        ..Lighting::default()
    };
    if output.extension().is_some_and(|e| e == "mp4") {
        use std::io::Write;
        let mut encoder = std::process::Command::new("ffmpeg")
            .args([
                "-y",
                "-loglevel",
                "error",
                "-f",
                "rawvideo",
                "-pixel_format",
                "rgba",
                "-video_size",
                "1600x560",
                "-framerate",
                "30",
                "-i",
                "pipe:0",
                "-an",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&output)
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        let mut input = encoder.stdin.take().ok_or("Missing video input")?;
        for (segment, (id, label)) in [
            (0, "Idle"),
            (4, "Walk"),
            (5, "Run"),
            (13, "Backpedal"),
            (14, "Strafe left"),
            (15, "Strafe right"),
            (25, "Guard"),
            (51, "Combat ready"),
            (52, "Spell windup"),
            (53, "Spell release"),
            (109, "Bow ready"),
            (46, "Bow release"),
            (1, "Fall and corpse"),
        ]
        .into_iter()
        .enumerate()
        {
            for frame in 0..90 {
                let time = frame as f32 / 30.;
                let mut lighting = lighting.clone();
                lighting.time = segment as f32 * 3. + time;
                for (i, actor) in instances.iter_mut().enumerate() {
                    actor.actor = Some(i as u64 + 1);
                    actor.animation = id;
                    actor.time = time;
                }
                let mut labels = ui.clone();
                labels.text(&atlas, 660., 45., label, [1., 0.85, 0.5, 1.]);
                let pixels = renderer.draw(view, &instances, &labels, &lighting)?;
                input.write_all(&pixels).map_err(|e| e.to_string())?;
            }
        }
        drop(input);
        if !encoder.wait().map_err(|e| e.to_string())?.success() {
            return Err("Animation recording failed".into());
        }
        std::fs::remove_dir_all(dir).map_err(|e| e.to_string())?;
        return Ok(());
    }
    let pixels = renderer.draw(view, &instances, &ui, &lighting)?;
    let mut encoder = png::Encoder::new(
        std::fs::File::create(output).map_err(|e| e.to_string())?,
        1600,
        560,
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .map_err(|e| e.to_string())?
        .write_image_data(&pixels)
        .map_err(|e| e.to_string())?;
    std::fs::remove_dir_all(dir).map_err(|e| e.to_string())?;
    Ok(())
}
