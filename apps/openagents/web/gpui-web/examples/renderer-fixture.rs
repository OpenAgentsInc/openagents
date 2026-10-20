//! WebGL2 primitive and atlas regression fixture. Compare app screenshots
//! against WebGPU with scripts/test-web-renderer.mjs.
#![cfg(target_family = "wasm")]
#[path = "../src/webgl.rs"]
mod webgl;
use gpui::*;
use std::{borrow::Cow, cell::RefCell};
use wasm_bindgen::prelude::*;

type Renderer = webgl::Renderer;
thread_local! { static RENDERER: RefCell<Option<Renderer>> = const { RefCell::new(None) }; }
fn bounds(x: f32, y: f32, w: f32, h: f32) -> Bounds<ScaledPixels> {
    Bounds {
        origin: point(ScaledPixels(x), ScaledPixels(y)),
        size: size(ScaledPixels(w), ScaledPixels(h)),
    }
}
fn glyph_key(subpixel: bool) -> AtlasKey {
    RenderGlyphParams {
        font_id: FontId(0),
        glyph_id: GlyphId(1),
        font_size: px(16.0),
        subpixel_variant: point(0, 0),
        scale_factor: 1.0,
        is_emoji: false,
        subpixel_rendering: subpixel,
        dilation: 0,
    }
    .into()
}
#[wasm_bindgen]
pub async fn draw(canvas: web_sys::HtmlCanvasElement, phase: u32) -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    openagents_gpui_web::init_logging();
    if RENDERER.with(|r| r.borrow().is_none()) {
        let renderer = Renderer::new(&canvas)?;
        RENDERER.with(|r| *r.borrow_mut() = Some(renderer));
    }
    RENDERER.with(|r| {
        let mut r = r.borrow_mut();
        let renderer = r.as_mut().unwrap();
        renderer.update_drawable_size(size(
            DevicePixels(canvas.width() as i32),
            DevicePixels(canvas.height() as i32),
        ));
        let atlas = renderer.sprite_atlas();
        let mut scene = Scene::default();
        let mask = ContentMask {
            bounds: bounds(0.0, 0.0, 640.0, 480.0),
        };
        let mut quad = |b: Bounds<ScaledPixels>,
                        background: Background,
                        style: BorderStyle,
                        radius: f32,
                        fade: EdgeFadeParams| {
            scene.insert_primitive(Quad {
                bounds: b,
                content_mask: mask,
                background,
                border_color: rgb(0x3666bb).into(),
                border_widths: Edges::all(ScaledPixels(3.0)),
                border_style: style,
                corner_radii: Corners::all(ScaledPixels(radius)),
                fade,
                ..Default::default()
            });
        };
        quad(
            bounds(0.0, 0.0, 640.0, 480.0),
            rgb(0xffffff).into(),
            BorderStyle::Solid,
            0.0,
            Default::default(),
        );
        quad(
            bounds(20.0, 20.0, 160.0, 100.0),
            rgba(0xbb441188).into(),
            BorderStyle::Solid,
            16.0,
            Default::default(),
        );
        quad(
            bounds(200.0, 20.0, 160.0, 100.0),
            linear_gradient(
                125.0,
                linear_color_stop(rgb(0xff2200), 0.1),
                linear_color_stop(rgb(0x0033ff), 0.9),
            ),
            BorderStyle::Solid,
            12.0,
            Default::default(),
        );
        quad(
            bounds(380.0, 20.0, 110.0, 100.0),
            linear_gradient(
                90.0,
                linear_color_stop(rgb(0xdd7700), 0.0),
                linear_color_stop(rgb(0x0044dd), 1.0),
            )
            .color_space(ColorSpace::Oklab),
            BorderStyle::Solid,
            0.0,
            Default::default(),
        );
        quad(
            bounds(20.0, 140.0, 160.0, 90.0),
            rgb(0xeeddbb).into(),
            BorderStyle::Dashed,
            0.0,
            Default::default(),
        );
        quad(
            bounds(200.0, 140.0, 160.0, 90.0),
            rgb(0xeeddbb).into(),
            BorderStyle::Dashed,
            18.0,
            Default::default(),
        );
        quad(
            bounds(380.0, 140.0, 200.0, 90.0),
            rgb(0xaa22bb).into(),
            BorderStyle::Solid,
            0.0,
            EdgeFadeParams {
                left_x: 380.0,
                right_x: 580.0,
                band_left: 60.0,
                band_right: 60.0,
                clip_left: 390.0,
                clip_top: 150.0,
                clip_right: 570.0,
                clip_bottom: 220.0,
                clip_radius: 18.0,
                ..Default::default()
            },
        );
        for (i, wavy) in [false, true].into_iter().enumerate() {
            scene.insert_primitive(Underline {
                order: 0,
                pad: 0,
                bounds: bounds(20.0, 250.0 + i as f32 * 20.0, 220.0, 12.0),
                content_mask: mask,
                color: rgb(0xcc2255).into(),
                thickness: ScaledPixels(2.0),
                wavy: wavy.into(),
            });
        }
        let key = AtlasKey::Image(RenderImageParams {
            image_id: ImageId(1),
            frame_index: 0,
        });
        if phase > 0 {
            atlas.remove(&key);
        }
        let image = atlas
            .get_or_insert_with(&key, &mut || {
                Ok(Some((
                    size(DevicePixels(8), DevicePixels(8)),
                    Cow::Owned(
                        (0..64)
                            .flat_map(|i| {
                                if phase == 0 {
                                    [40, (i % 8 * 28) as u8, 220, 255]
                                } else {
                                    [220, 30, 40, 255]
                                }
                            })
                            .collect(),
                    ),
                )))
            })
            .unwrap()
            .unwrap();
        scene.insert_primitive(PolychromeSprite {
            order: 0,
            pad: 0,
            grayscale: false.into(),
            opacity: 0.8,
            bounds: bounds(280.0, 250.0, 90.0, 80.0),
            content_mask: mask,
            corner_radii: Corners::all(ScaledPixels(12.0)),
            fade: Default::default(),
            alpha_mask: Default::default(),
            tile: image,
        });
        for (i, subpixel) in [false, true].into_iter().enumerate() {
            let key = glyph_key(subpixel);
            let pixels: Vec<u8> = (0..64)
                .flat_map(|n| {
                    let c = if n % 8 > 1 && n % 8 < 6 { 200 } else { 0 };
                    if subpixel {
                        vec![c, c, c, 255]
                    } else {
                        vec![c]
                    }
                })
                .collect();
            let tile = atlas
                .get_or_insert_with(&key, &mut || {
                    Ok(Some((
                        size(DevicePixels(8), DevicePixels(8)),
                        Cow::Borrowed(&pixels),
                    )))
                })
                .unwrap()
                .unwrap();
            let b = bounds(400.0 + i as f32 * 30.0, 260.0, 8.0, 8.0);
            if subpixel {
                scene.insert_primitive(SubpixelSprite {
                    order: 0,
                    pad: 0,
                    bounds: b,
                    content_mask: mask,
                    color: rgb(0x224466).into(),
                    tile,
                    transformation: Default::default(),
                    fade: Default::default(),
                    blur: 0.0,
                    pad2: 0.0,
                });
            } else {
                scene.insert_primitive(MonochromeSprite {
                    order: 0,
                    pad: 0,
                    bounds: b,
                    content_mask: mask,
                    color: rgb(0x224466).into(),
                    tile,
                    transformation: Default::default(),
                    fade: Default::default(),
                    blur: 0.0,
                    pad2: 0.0,
                });
            }
        }
        let mut builder = PathBuilder::fill();
        builder.move_to(point(px(30.0), px(330.0)));
        builder.line_to(point(px(180.0), px(330.0)));
        builder.curve_to(point(px(90.0), px(430.0)), point(px(180.0), px(430.0)));
        builder.close();
        let mut path = builder.build().unwrap().scale(1.0);
        path.color = rgb(0x6688dd).into();
        path.content_mask = mask;
        scene.insert_primitive(path);
        scene.finish();
        assert!(
            renderer.draw(&scene),
            "renderer did not present the fixture"
        );
    });
    Ok(())
}
