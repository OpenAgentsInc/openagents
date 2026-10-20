//! The one shared terminal and its local message draft. No delivery yet.

use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Document, Element, HtmlElement, HtmlInputElement, KeyboardEvent};

use crate::{terminal, world_camera::WorldCamera};

pub struct Terminal {
    svg: Element,
    paths: Vec<Element>,
    panel: HtmlElement,
    input: HtmlInputElement,
    hint: HtmlElement,
    near: bool,
    faces: Vec<[[f64; 3]; 4]>,
}

fn style(element: &HtmlElement, properties: &[(&str, &str)]) {
    for (name, value) in properties {
        let _ = element.style().set_property(name, value);
    }
}

impl Terminal {
    /// Only /home opts in. Other pages keep their existing controls.
    pub fn new(document: &Document) -> Option<Self> {
        let window = document.default_view()?;
        if window.location().pathname().ok()?.trim_end_matches('/') != "/home" {
            return None;
        }
        let body = document.body()?;
        let create = |tag| {
            document
                .create_element(tag)
                .ok()?
                .dyn_into::<HtmlElement>()
                .ok()
        };
        let svg = document
            .create_element_ns(Some("http://www.w3.org/2000/svg"), "svg")
            .ok()?;
        svg.set_attribute("id", "pylons-terminal").ok()?;
        svg.set_attribute("aria-hidden", "true").ok()?;
        // The SVG itself has no hit box; the world canvas owns looking and walking.
        // CSSOM styles work under the page's strict CSP, like pillar positioning.
        let svg_style = js_sys::Reflect::get(&svg, &"style".into())
            .ok()?
            .dyn_into::<web_sys::CssStyleDeclaration>()
            .ok()?;
        svg_style.set_css_text("position:fixed;inset:0;width:100%;height:100%;z-index:2;pointer-events:none;visibility:hidden");
        let faces = terminal::faces();
        let mut paths = Vec::with_capacity(faces.len());
        for _ in &faces {
            let path = document
                .create_element_ns(Some("http://www.w3.org/2000/svg"), "path")
                .ok()?;
            path.set_attribute("fill", "#080808").ok()?;
            path.set_attribute("stroke", "#d0d0d0").ok()?;
            path.set_attribute("stroke-width", "1").ok()?;
            svg.append_child(&path).ok()?;
            paths.push(path);
        }
        body.append_child(&svg).ok()?;

        let panel = create("section")?;
        panel.set_id("pylon-composer");
        panel
            .set_attribute("aria-label", "Message the pylons")
            .ok()?;
        panel.set_attribute("aria-hidden", "true").ok()?;
        panel.set_attribute("inert", "").ok()?;
        style(
            &panel,
            &[
                ("position", "fixed"),
                ("z-index", "6"),
                ("bottom", "max(20px, env(safe-area-inset-bottom))"),
                ("left", "50%"),
                ("width", "min(640px, calc(100vw - 32px))"),
                ("box-sizing", "border-box"),
                ("padding", "14px 18px"),
                ("border-radius", "16px"),
                ("border", "1px solid #444"),
                ("background", "rgba(10,10,10,.94)"),
                ("box-shadow", "0 12px 40px #0008"),
                ("opacity", "0"),
                ("visibility", "hidden"),
                ("transform", "translate(-50%, 12px)"),
            ],
        );
        let still = window
            .match_media("(prefers-reduced-motion: reduce)")
            .ok()
            .flatten()
            .is_some_and(|m| m.matches());
        if !still {
            style(
                &panel,
                &[(
                    "transition",
                    "opacity .3s ease, transform .3s ease, visibility .3s",
                )],
            );
        }
        let label = create("label")?;
        label.set_attribute("for", "pylon-message").ok()?;
        label.set_text_content(Some("Message the pylons"));
        style(
            &label,
            &[
                ("display", "block"),
                ("font-size", "12px"),
                ("color", "#ccc"),
                ("margin-bottom", "8px"),
            ],
        );
        panel.append_child(&label).ok()?;
        let input = document
            .create_element("input")
            .ok()?
            .dyn_into::<HtmlInputElement>()
            .ok()?;
        input.set_id("pylon-message");
        input.set_type("text");
        input.set_max_length(4000);
        input.set_placeholder("Type a message…");
        input.set_autocomplete("off");
        input.set_disabled(true);
        input
            .set_attribute("aria-describedby", "pylon-draft-note")
            .ok()?;
        style(
            input.unchecked_ref(),
            &[
                ("display", "block"),
                ("width", "100%"),
                ("box-sizing", "border-box"),
                ("font", "inherit"),
                ("font-size", "16px"),
                ("color", "#fff"),
                ("background", "#151515"),
                ("border", "1px solid #555"),
                ("border-radius", "8px"),
                ("padding", "12px"),
                ("outline-offset", "3px"),
            ],
        );
        panel.append_child(&input).ok()?;
        let note = create("small")?;
        note.set_id("pylon-draft-note");
        note.set_text_content(Some("Draft · kept in this tab"));
        style(
            &note,
            &[
                ("display", "block"),
                ("font-size", "11px"),
                ("color", "#999"),
                ("margin-top", "7px"),
            ],
        );
        panel.append_child(&note).ok()?;
        body.append_child(&panel).ok()?;

        let hint = create("p")?;
        hint.set_id("grid-controls");
        hint.set_text_content(Some("WASD to walk · Drag to look"));
        style(
            &hint,
            &[
                ("position", "fixed"),
                ("bottom", "20px"),
                ("left", "50%"),
                ("transform", "translateX(-50%)"),
                ("z-index", "3"),
                ("margin", "0"),
                ("padding", "8px 12px"),
                ("background", "#080808cc"),
                ("border-radius", "8px"),
                ("font-size", "12px"),
                ("color", "#aaa"),
                ("white-space", "nowrap"),
                ("pointer-events", "none"),
            ],
        );
        body.append_child(&hint).ok()?;
        let canvas = document.get_element_by_id("everglade-canvas")?;
        let input_key = input.clone();
        let key = Closure::<dyn FnMut(KeyboardEvent)>::new(move |event: KeyboardEvent| {
            if event.key() == "Escape" {
                event.prevent_default();
                event.stop_propagation();
                let _ = input_key.blur();
                if let Some(canvas) = canvas.dyn_ref::<HtmlElement>() {
                    let _ = canvas.focus();
                }
            }
        });
        input
            .add_event_listener_with_callback("keydown", key.as_ref().unchecked_ref())
            .ok()?;
        key.forget();
        Some(Self {
            svg,
            paths,
            panel,
            input,
            hint,
            near: false,
            faces,
        })
    }

    pub fn frame(&mut self, camera: Option<WorldCamera>, player: Option<[f64; 3]>) {
        let near = camera.is_some() && player.is_some_and(|p| terminal::in_reach(p, self.near));
        if near != self.near {
            self.near = near;
            if !near {
                let _ = self.input.blur();
            }
            self.input.set_disabled(!near);
            let _ = self
                .panel
                .set_attribute("aria-hidden", if near { "false" } else { "true" });
            if near {
                let _ = self.panel.remove_attribute("inert");
            } else {
                let _ = self.panel.set_attribute("inert", "");
            }
            style(
                &self.panel,
                &[
                    ("opacity", if near { "1" } else { "0" }),
                    ("visibility", if near { "visible" } else { "hidden" }),
                    (
                        "transform",
                        if near {
                            "translate(-50%, 0)"
                        } else {
                            "translate(-50%, 12px)"
                        },
                    ),
                ],
            );
            style(
                &self.hint,
                &[("visibility", if near { "hidden" } else { "visible" })],
            );
        }
        let mut faces = Vec::new();
        if let Some(camera) = camera {
            for face in &self.faces {
                let points: Option<Vec<_>> = face.iter().map(|p| camera.project(*p)).collect();
                if let Some(points) = points {
                    let depth = points.iter().map(|p| p.2).sum::<f64>();
                    let mut path = String::new();
                    for (i, (x, y, _)) in points.into_iter().enumerate() {
                        use std::fmt::Write;
                        let _ = write!(path, "{}{x:.2},{y:.2}", if i == 0 { 'M' } else { 'L' });
                    }
                    path.push('Z');
                    faces.push((depth, path));
                }
            }
        }
        // Reuse the nodes; only their projected points change per frame.
        faces.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (i, path) in self.paths.iter().enumerate() {
            let d = faces.get(i).map_or("", |f| f.1.as_str());
            let _ = path.set_attribute("d", d);
        }
        if let Ok(style) = js_sys::Reflect::get(&self.svg, &"style".into())
            .and_then(|s| s.dyn_into::<web_sys::CssStyleDeclaration>())
        {
            let _ = style.set_property(
                "visibility",
                if camera.is_some() {
                    "visible"
                } else {
                    "hidden"
                },
            );
        }
    }
}
