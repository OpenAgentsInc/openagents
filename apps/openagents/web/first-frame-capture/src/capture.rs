//! Runs the AgentGraph view headless and records its first frame.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use anyhow::{Context as _, Result, anyhow, ensure};
use gpui::{
    AppContext as _, Application, Bounds, PlatformTextSystem, RequestFrameOptions, Size,
    TestDispatcher, WindowBounds, WindowOptions, point, px, size,
};
use openagents_first_frame as ff;
pub use openagents_first_frame_record::Frame;
use openagents_first_frame_record::{Recorder, convert};

use crate::platform::{CapturePlatform, Surface};

/// Lays out and paints the graph `json` in a window of `frame`'s size and
/// scale and returns its first frame.
pub fn capture(json: &str, frame: Frame) -> Result<ff::Scene> {
    ensure!(
        frame.size.0 >= 1.0
            && frame.size.1 >= 1.0
            && frame.size.0 <= 8192.0
            && frame.size.1 <= 8192.0,
        "the window size is out of range"
    );
    ensure!(
        frame.scale > 0.0 && frame.scale <= 4.0,
        "the scale is out of range"
    );
    let spec = openagents_web_agent_graph::parse(json).map_err(|e| anyhow!(e))?;
    let text = Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts(
        openagents_web_agent_graph::theme::SANS,
    ));
    text.add_fonts(openagents_web_agent_graph::assets::font_faces())?;
    let atlas = Arc::new(Recorder::default());
    let out: Rc<RefCell<Option<Result<ff::Scene>>>> = Rc::default();
    let present = {
        let (atlas, out) = (atlas.clone(), out.clone());
        Rc::new(move |scene: &gpui::Scene, _: Size<gpui::Pixels>, _: f32| {
            let mut slot = out.borrow_mut();
            if slot.is_none() {
                *slot = Some(convert(scene, frame.device(), frame.scale, &atlas));
            }
        })
    };
    let dispatcher = TestDispatcher::new(0);
    let platform = Rc::new(CapturePlatform::new(
        &dispatcher,
        text as Arc<dyn PlatformTextSystem>,
        Surface {
            scale: frame.scale,
            subpixel: frame.subpixel,
            atlas: atlas.clone(),
            present,
        },
    ));
    let request = platform.frame();
    let handle = Application::with_platform(platform).run_embedded(move |cx| {
        let bounds = Bounds {
            origin: point(px(0.0), px(0.0)),
            size: size(px(frame.size.0), px(frame.size.1)),
        };
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                focus: false,
                show: false,
                ..Default::default()
            },
            move |_, cx| {
                cx.new(|_| openagents_web_agent_graph::GraphView::new(spec, false, Vec::new()))
            },
        )
        .expect("a capture window opens");
    });
    dispatcher.run_until_parked();
    // The frame the browser shows first: GPUI's first presented frame.
    let mut callback = request
        .borrow_mut()
        .take()
        .context("the window asked for no frames")?;
    callback(RequestFrameOptions {
        require_presentation: true,
        force_render: true,
    });
    dispatcher.run_until_parked();
    drop(callback);
    drop(handle);
    let scene = out
        .borrow_mut()
        .take()
        .context("the window drew no frame")?;
    scene
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real graph at both classes and at other scales: deterministic
    /// and drawable.
    #[test]
    fn foremans_first_frame_is_deterministic_and_round_trips() {
        let json = serde_json::to_string(&agent_graph::foreman::graph()).unwrap();
        for class in &crate::CLASSES {
            let frame = Frame {
                size: class.size,
                scale: 2.0,
                subpixel: true,
            };
            let a = capture(&json, frame).unwrap();
            let b = capture(&json, frame).unwrap();
            assert_eq!(a, b, "{} differs between runs", class.name);
            assert_eq!(
                (a.width, a.height),
                (
                    (class.size.0 * 2.0).round() as u32,
                    (class.size.1 * 2.0).round() as u32
                )
            );
            assert!(!a.quads.is_empty() && !a.sprites.is_empty() && !a.paths.is_empty());
            assert_eq!(a.atlas.channels, 3);
            assert!(
                a.commands
                    .iter()
                    .any(|c| matches!(c, ff::Command::SubpixelSprites(_)))
            );
            let bytes = a.encode().unwrap();
            assert_eq!(ff::Scene::decode(&bytes).unwrap(), a);
        }
        // A phone at 3x and a plain screen at 1x, with grayscale text.
        for scale in [1.0, 3.0] {
            let frame = Frame {
                size: (375.0, 600.5),
                scale,
                subpixel: false,
            };
            let s = capture(&json, frame).unwrap();
            assert_eq!(
                (s.width, s.height),
                ((375.0 * scale) as u32, (600.5 * scale).round() as u32)
            );
            assert_eq!(s.atlas.channels, 1);
            assert!(
                s.commands
                    .iter()
                    .any(|c| matches!(c, ff::Command::Sprites(_)))
            );
        }
        assert!(
            capture(
                &json,
                Frame {
                    size: (0.0, 10.0),
                    scale: 2.0,
                    subpixel: true
                }
            )
            .is_err()
        );
        assert!(
            capture(
                &json,
                Frame {
                    size: (10.0, 10.0),
                    scale: 9.0,
                    subpixel: true
                }
            )
            .is_err()
        );
    }
}
