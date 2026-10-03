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
            let transform = Mat4::from_translation(a.actor.position)
                * Mat4::from_rotation_y(a.actor.yaw)
                * basis()
                * pose[hand.bone]
                * Mat4::from_translation(hand.position.into())
                * Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2)
                * Mat4::from_rotation_y(-std::f32::consts::FRAC_PI_2);
            actors.push(Instance {
                model: "bow".into(),
                transform,
                animation: 0,
                time,
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
            time,
            emission: Vec3::ONE,
        });
    }
    let heights: std::collections::BTreeMap<_, _> = pack
        .models
        .iter()
        .map(|(id, m)| (id.clone(), m.height))
        .collect();
    let atlas = Atlas::new(16.0);
    let mut renderer = Renderer::new(
        pack,
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
    let pixels = renderer.draw(view, &actors, &ui, &lighting)?;
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
