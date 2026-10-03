//! Capture an imported WoW chamber with Verse-owned textured GPU rendering.
use glam::{Mat4, Quat, Vec3};
use std::{io::Write, path::PathBuf};
use verse::{
    imported::{Instance, Renderer},
    render::View,
    ui::{Atlas, UiBatch},
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
    let mut actors = vec![Instance {
        model: "claude".into(),
        transform: conversion
            * Mat4::from_scale_rotation_translation(
                Vec3::splat(2.4),
                Quat::IDENTITY,
                [-15.0, 141.0, 83.9].into(),
            ),
        animation: 0,
        time: 0.0,
        emission: Vec3::new(0.08, 1.0, 0.03),
    }];
    for i in 0..12 {
        let a = i as f32 * std::f32::consts::TAU / 12.0;
        actors.push(Instance {
            model: "cultist".into(),
            transform: conversion
                * Mat4::from_scale_rotation_translation(
                    Vec3::ONE,
                    Quat::from_rotation_z(a + std::f32::consts::PI),
                    [-15.0 + 9.0 * a.cos(), 141.0 + 9.0 * a.sin(), 83.9].into(),
                ),
            animation: 0,
            time: i as f32 * 0.1,
            emission: Vec3::ONE,
        });
    }
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
    let eye = position_from_wow([15.0, 141.0, 86.0]) - origin;
    let view = View {
        view_proj: Mat4::perspective_rh(1.0, 1280.0 / 720.0, 0.1, 500.0)
            * Mat4::look_at_rh(eye, Vec3::new(0.0, 3.0, 0.0), Vec3::Y),
        eye,
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
    let pixels = renderer.draw(view, &actors, &UiBatch::default(), &lighting)?;
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
