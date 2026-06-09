#[cfg(target_arch = "wasm32")]
use std::rc::Rc;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::closure::Closure;
#[cfg(target_arch = "wasm32")]
use web_sys::{HtmlCanvasElement, HtmlElement, KeyboardEvent, MouseEvent, WheelEvent};
#[cfg(target_arch = "wasm32")]
use wgpui::{InputEvent, Key, Modifiers, MouseButton, NamedKey};

#[cfg(target_arch = "wasm32")]
pub(crate) fn install_input_bridge(
    canvas: HtmlCanvasElement,
    on_input: Rc<dyn Fn(InputEvent)>,
) -> Result<(), String> {
    let window = web_sys::window().ok_or("window is not available")?;

    {
        let on_input = on_input.clone();
        let keydown = Closure::<dyn FnMut(KeyboardEvent)>::new(move |event: KeyboardEvent| {
            if let Some(key) = map_key(event.key().as_str()) {
                if consumes_default_shortcut(&key) {
                    event.prevent_default();
                }

                on_input(InputEvent::KeyDown {
                    key,
                    modifiers: keyboard_modifiers(&event),
                });
            }
        });

        let result =
            window.add_event_listener_with_callback("keydown", keydown.as_ref().unchecked_ref());
        if let Err(error) = result {
            return Err(format!("failed to attach keydown listener: {error:?}"));
        }
        keydown.forget();
    }

    {
        let on_input = on_input.clone();
        let keyup = Closure::<dyn FnMut(KeyboardEvent)>::new(move |event: KeyboardEvent| {
            if let Some(key) = map_key(event.key().as_str()) {
                if consumes_default_shortcut(&key) {
                    event.prevent_default();
                }

                on_input(InputEvent::KeyUp {
                    key,
                    modifiers: keyboard_modifiers(&event),
                });
            }
        });

        let result =
            window.add_event_listener_with_callback("keyup", keyup.as_ref().unchecked_ref());
        if let Err(error) = result {
            return Err(format!("failed to attach keyup listener: {error:?}"));
        }
        keyup.forget();
    }

    {
        let canvas_ref = canvas.clone();
        let on_input = on_input.clone();
        let mousemove = Closure::<dyn FnMut(MouseEvent)>::new(move |event| {
            let point = canvas_point(&canvas_ref, &event);
            on_input(InputEvent::MouseMove {
                x: point.0,
                y: point.1,
            });
        });

        let result = canvas
            .add_event_listener_with_callback("mousemove", mousemove.as_ref().unchecked_ref());
        if let Err(error) = result {
            return Err(format!("failed to attach mousemove listener: {error:?}"));
        }
        mousemove.forget();
    }

    {
        let canvas_ref = canvas.clone();
        let on_input = on_input.clone();
        let mousedown = Closure::<dyn FnMut(MouseEvent)>::new(move |event: MouseEvent| {
            let Some(button) = map_mouse_button(event.button()) else {
                return;
            };

            let point = canvas_point(&canvas_ref, &event);
            on_input(InputEvent::MouseDown {
                button,
                x: point.0,
                y: point.1,
                modifiers: mouse_modifiers(&event),
            });
        });

        let result = canvas
            .add_event_listener_with_callback("mousedown", mousedown.as_ref().unchecked_ref());
        if let Err(error) = result {
            return Err(format!("failed to attach mousedown listener: {error:?}"));
        }
        mousedown.forget();
    }

    {
        let canvas_ref = canvas.clone();
        let on_input = on_input.clone();
        let mouseup = Closure::<dyn FnMut(MouseEvent)>::new(move |event: MouseEvent| {
            let Some(button) = map_mouse_button(event.button()) else {
                return;
            };

            let point = canvas_point(&canvas_ref, &event);
            on_input(InputEvent::MouseUp {
                button,
                x: point.0,
                y: point.1,
            });
        });

        let result =
            canvas.add_event_listener_with_callback("mouseup", mouseup.as_ref().unchecked_ref());
        if let Err(error) = result {
            return Err(format!("failed to attach mouseup listener: {error:?}"));
        }
        mouseup.forget();
    }

    {
        let on_input = on_input.clone();
        let wheel = Closure::<dyn FnMut(WheelEvent)>::new(move |event: WheelEvent| {
            event.prevent_default();
            on_input(InputEvent::Scroll {
                dx: event.delta_x() as f32,
                dy: event.delta_y() as f32,
            });
        });

        let result =
            canvas.add_event_listener_with_callback("wheel", wheel.as_ref().unchecked_ref());
        if let Err(error) = result {
            return Err(format!("failed to attach wheel listener: {error:?}"));
        }
        wheel.forget();
    }

    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn map_key(raw: &str) -> Option<Key> {
    match raw {
        "ArrowLeft" => Some(Key::Named(NamedKey::ArrowLeft)),
        "ArrowRight" => Some(Key::Named(NamedKey::ArrowRight)),
        "ArrowUp" => Some(Key::Named(NamedKey::ArrowUp)),
        "ArrowDown" => Some(Key::Named(NamedKey::ArrowDown)),
        "Home" => Some(Key::Named(NamedKey::Home)),
        "End" => Some(Key::Named(NamedKey::End)),
        "PageUp" => Some(Key::Named(NamedKey::PageUp)),
        "PageDown" => Some(Key::Named(NamedKey::PageDown)),
        "Backspace" => Some(Key::Named(NamedKey::Backspace)),
        "Enter" => Some(Key::Named(NamedKey::Enter)),
        "Escape" => Some(Key::Named(NamedKey::Escape)),
        " " | "Spacebar" => Some(Key::Named(NamedKey::Space)),
        other if other.chars().count() == 1 => Some(Key::Character(other.to_string())),
        _ => None,
    }
}

#[cfg(target_arch = "wasm32")]
fn consumes_default_shortcut(key: &Key) -> bool {
    matches!(
        key,
        Key::Named(
            NamedKey::ArrowLeft
                | NamedKey::ArrowRight
                | NamedKey::ArrowUp
                | NamedKey::ArrowDown
                | NamedKey::Home
                | NamedKey::End
                | NamedKey::PageUp
                | NamedKey::PageDown
                | NamedKey::Backspace
                | NamedKey::Space
        )
    )
}

#[cfg(target_arch = "wasm32")]
fn map_mouse_button(raw: i16) -> Option<MouseButton> {
    match raw {
        0 => Some(MouseButton::Left),
        1 => Some(MouseButton::Middle),
        2 => Some(MouseButton::Right),
        _ => None,
    }
}

#[cfg(target_arch = "wasm32")]
fn keyboard_modifiers(event: &KeyboardEvent) -> Modifiers {
    Modifiers {
        shift: event.shift_key(),
        ctrl: event.ctrl_key(),
        alt: event.alt_key(),
        meta: event.meta_key(),
    }
}

#[cfg(target_arch = "wasm32")]
fn mouse_modifiers(event: &MouseEvent) -> Modifiers {
    Modifiers {
        shift: event.shift_key(),
        ctrl: event.ctrl_key(),
        alt: event.alt_key(),
        meta: event.meta_key(),
    }
}

#[cfg(target_arch = "wasm32")]
fn canvas_point(canvas: &HtmlCanvasElement, event: &MouseEvent) -> (f32, f32) {
    let rect = canvas.get_bounding_client_rect();
    (
        (f64::from(event.client_x()) - rect.left()) as f32,
        (f64::from(event.client_y()) - rect.top()) as f32,
    )
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn clear_boot_status() {
    set_boot_status(None);
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn report_boot_error(message: &str) {
    web_sys::console::error_1(&message.into());
    set_boot_status(Some(message));
}

#[cfg(target_arch = "wasm32")]
fn set_boot_status(message: Option<&str>) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let Some(element) = document.get_element_by_id("boot-status") else {
        return;
    };
    let Ok(status) = element.dyn_into::<HtmlElement>() else {
        return;
    };

    match message {
        Some(message) => {
            status.set_hidden(false);
            status.set_inner_text(message);
        }
        None => {
            status.set_hidden(true);
            status.set_inner_text("");
        }
    }
}
