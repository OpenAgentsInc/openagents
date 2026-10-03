//! Capture an imported WoW chamber with Verse-owned textured GPU rendering.
use glam::{Mat4, Quat, Vec3};
use std::{io::Write, path::PathBuf};
use verse::{
    imported::{Instance, Renderer},
    render::View,
    ui::Atlas,
};
use verse_wow::{assets::Pack, position_from_wow};
fn basis() -> Mat4 {
    Mat4::from_cols_array(&[
        0.0, 0.0, -0.9144, 0.0, -0.9144, 0.0, 0.0, 0.0, 0.0, 0.9144, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ])
}
fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let path = PathBuf::from(args.next().ok_or("Expected private pack.json")?);
    let output = PathBuf::from(args.next().ok_or("Expected output.png")?);
    let mode = args.next().unwrap_or_default();
    let pack = Pack::read(&path)?;
    let origin = position_from_wow([-15.0, 141.0, 83.9]);
    let conversion = Mat4::from_translation(-origin) * basis();
    let static_instances: Vec<_> = pack
        .placements
        .iter()
        .map(|p| Instance {
            model: p.model.clone(),
            transform: conversion
                * Mat4::from_scale_rotation_translation(
                    Vec3::splat(p.scale),
                    Quat::from_array(p.rotation),
                    p.position.into(),
                ),
            animation: 0,
            time: 0.0,
            emission: if pack.models[&p.model]
                .source
                .ends_with("scholme_greencandelabra.m2")
            {
                Vec3::new(1.0, 0.38, 0.1)
            } else {
                Vec3::new(0.08, 1.0, 0.03)
            },
        })
        .collect();
    let scene = verse_wow::director::Scene::from_json(include_bytes!(
        "../../../assets/verse/wow/anthropic.json"
    ))?;
    let time: f32 = mode.parse().unwrap_or(3.0);
    let frame = scene.frame(time);
    let heights: std::collections::BTreeMap<_, _> = pack
        .models
        .iter()
        .map(|(id, m)| (id.clone(), m.height))
        .collect();
    let atlas = classic_atlas(path.parent().ok_or("Expected pack directory")?)?;
    let mut renderer = Renderer::new(
        pack.clone(),
        path.parent().ok_or("Expected pack directory")?,
        1280,
        720,
        &atlas,
        &static_instances,
    )?;
    eprintln!("Verse GPU: {}", renderer.adapter_name);
    let view = View {
        view_proj: frame.view_projection(1280.0 / 720.0),
        eye: frame.eye,
    };
    let mut lighting = verse::imported::lighting::Lighting::default();
    lighting.ambient = Vec3::new(0.055, 0.06, 0.075);
    lighting.exposure = 1.35;
    for (p, c, intensity) in [
        ([-4.1, 124.2, 87.0], [1.0, 0.38, 0.1], 450.0),
        ([-4.1, 160.7, 88.0], [1.0, 0.38, 0.1], 450.0),
        ([-26.66, 138.575, 86.4], [0.18, 0.8, 0.12], 80.0),
        ([-26.5, 144.56, 86.4], [0.18, 0.8, 0.12], 80.0),
        ([19.066, 133.143, 86.4], [1.0, 0.38, 0.1], 250.0),
        ([18.752, 151.100, 86.4], [1.0, 0.38, 0.1], 250.0),
    ] {
        lighting.lights.push(verse::imported::lighting::Light {
            position: position_from_wow(p) - origin,
            color: c.into(),
            intensity,
            range: 38.0,
        });
    }
    if mode == "--no-shadows" {
        lighting.shadowed = 0;
    }
    if mode == "--lights-off" {
        lighting.lights.clear();
    }
    lighting.time = time;
    let ui = verse::imported::overlay::cinematic(
        &atlas,
        &frame,
        &heights,
        view.view_proj,
        1280.0,
        720.0,
    );
    if output.extension().is_some_and(|e| e == "mp4") {
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
                "1280x720",
                "-framerate",
                "30",
                "-i",
                "pipe:0",
                "-an",
                "-c:v",
                "libx264",
                "-preset",
                "fast",
                "-crf",
                "20",
                "-pix_fmt",
                "yuv420p",
                "-movflags",
                "+faststart",
            ])
            .arg(&output)
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        let mut pipe = encoder.stdin.take().ok_or("Missing video encoder input")?;
        for index in 0..(scene.duration * 30.0) as u32 {
            let frame = scene.frame(index as f32 / 30.0);
            lighting.time = frame.time;
            let view = View {
                view_proj: frame.view_projection(1280.0 / 720.0),
                eye: frame.eye,
            };
            let ui = verse::imported::overlay::cinematic(
                &atlas,
                &frame,
                &heights,
                view.view_proj,
                1280.0,
                720.0,
            );
            let pixels = renderer.draw(view, &instances(&pack, &frame), &ui, &lighting)?;
            pipe.write_all(&pixels).map_err(|e| e.to_string())?;
            if index % 300 == 0 {
                eprintln!("Captured {} / {} seconds", frame.time, scene.duration);
            }
        }
        drop(pipe);
        if !encoder.wait().map_err(|e| e.to_string())?.success() {
            return Err("Video encoder failed".into());
        }
        let evidence = serde_json::json!({
            "schema":"openagents.verse-wow.capture.v1",
            "renderer":"verse::imported (Rust/wgpu)",
            "adapter":renderer.adapter_name,
            "duration_seconds":scene.duration,
            "fps":30,
            "frames":(scene.duration*30.0) as u32,
            "resolution":[1280,720],
            "lighting":"engine point lights, cube shadows, distance fog",
            "dialogue":"simulation cues rendered in GPU overlay",
            "camera_cut_seconds":scene.cut_at,
            "yells":scene.cues.iter().filter(|c|matches!(c.action,verse_wow::director::Action::Yell{..})).count(),
            "bow_shots":scene.frame(scene.duration).shots.len(),
            "hostile_nameplates":scene.actors.iter().filter(|a|a.nameplate).count(),
            "source_revision":pack.source_revision,
        });
        std::fs::write(
            output.with_extension("json"),
            serde_json::to_vec_pretty(&evidence).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        return Ok(());
    }
    let pixels = renderer.draw(view, &instances(&pack, &frame), &ui, &lighting)?;
    let mut png = png::Encoder::new(
        std::fs::File::create(output).map_err(|e| e.to_string())?,
        1280,
        720,
    );
    png.set_color(png::ColorType::Rgba);
    png.set_depth(png::BitDepth::Eight);
    png.write_header()
        .map_err(|e| e.to_string())?
        .write_image_data(&pixels)
        .map_err(|e| e.to_string())?;
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    Ok(())
}

fn instances(pack: &Pack, frame: &verse_wow::director::Frame) -> Vec<Instance> {
    let mut actors: Vec<_> = frame
        .actors
        .iter()
        .filter(|a| a.visible)
        .map(|a| Instance {
            model: a.actor.model.clone(),
            transform: Mat4::from_translation(a.actor.position)
                * Mat4::from_rotation_y(a.actor.yaw)
                * Mat4::from_scale(Vec3::splat(a.actor.scale))
                * basis(),
            animation: a.animation,
            time: a.animation_time,
            emission: Vec3::ONE,
        })
        .collect();
    for a in frame
        .actors
        .iter()
        .filter(|a| a.visible && a.actor.model == "adventurer")
    {
        let model = &pack.models["adventurer"];
        if let Some(hand) = model.attachments.iter().find(|a| a.id == 2) {
            let pose = verse_wow::animation::pose(model, a.animation, a.animation_time);
            let hand_position = (Mat4::from_translation(a.actor.position)
                * Mat4::from_rotation_y(a.actor.yaw)
                * basis()
                * pose[hand.bone])
                .transform_point3(hand.position.into());
            let transform = Mat4::from_translation(hand_position)
                * Mat4::from_rotation_y(a.actor.yaw)
                * basis()
                * Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2)
                * Mat4::from_rotation_y(-std::f32::consts::FRAC_PI_2);
            actors.push(Instance {
                model: "bow".into(),
                transform,
                animation: 0,
                time: frame.time,
                emission: Vec3::ONE,
            });
        }
    }
    for arrow in &frame.projectiles {
        actors.push(Instance {
            model: "arrow".into(),
            transform: Mat4::from_translation(arrow.position)
                * Mat4::from_quat(Quat::from_rotation_arc(-Vec3::Z, arrow.direction))
                * basis(),
            animation: 0,
            time: frame.time,
            emission: Vec3::ONE,
        });
    }
    actors
}

fn classic_atlas(dir: &std::path::Path) -> Result<Atlas, String> {
    let font = std::fs::read(dir.join("FRIZQT__.TTF"))
        .map_err(|e| format!("Import Classic UI assets with wow-import --ui-only: {e}"))?;
    let mut atlas = Atlas::from_font(&font, 18.0)?;
    for name in ["status-bar", "nameplate-border"] {
        let decoder = png::Decoder::new(std::io::BufReader::new(
            std::fs::File::open(dir.join(format!("{name}.png"))).map_err(|e| e.to_string())?,
        ));
        let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
        let mut rgba = vec![0; reader.output_buffer_size().ok_or("Invalid UI image")?];
        let info = reader.next_frame(&mut rgba).map_err(|e| e.to_string())?;
        if info.color_type != png::ColorType::Rgba {
            return Err("Expected RGBA UI sprite".into());
        }
        if name == "nameplate-border" {
            if info.height != 32 || info.width != 128 {
                return Err("Unexpected Classic nameplate dimensions".into());
            }
            atlas.add_sprite(name, 128, 16, &rgba[128 * 16 * 4..128 * 32 * 4])?;
        } else {
            atlas.add_sprite(name, info.width, info.height, &rgba[..info.buffer_size()])?;
        }
    }
    Ok(atlas)
}
