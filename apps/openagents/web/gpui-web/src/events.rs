use std::{cell::RefCell, rc::Rc};

use gpui::{
    Capslock, DispatchEventResult, ExternalPaths, FileDropEvent, KeyDownEvent, KeyUpEvent,
    Keystroke, Modifiers, ModifiersChangedEvent, MouseButton, MouseDownEvent, MouseExitEvent,
    MouseMoveEvent, MouseUpEvent, NavigationDirection, Pixels, PlatformInput, Point, ScrollDelta,
    ScrollWheelEvent, TouchPhase, point, px,
};
use smallvec::smallvec;
use wasm_bindgen::prelude::*;

use crate::window::WebWindowInner;

pub struct WebEventListeners {
    #[allow(dead_code)]
    closures: Vec<Closure<dyn FnMut(JsValue)>>,
}

pub(crate) struct ClickState {
    last_position: Point<Pixels>,
    last_time: f64,
    current_count: usize,
}

impl Default for ClickState {
    fn default() -> Self {
        Self {
            last_position: Point::default(),
            last_time: 0.0,
            current_count: 0,
        }
    }
}

impl ClickState {
    fn register_click(&mut self, position: Point<Pixels>, time: f64) -> usize {
        let distance = ((f32::from(position.x) - f32::from(self.last_position.x)).powi(2)
            + (f32::from(position.y) - f32::from(self.last_position.y)).powi(2))
        .sqrt();

        if (time - self.last_time) < 400.0 && distance < 5.0 {
            self.current_count += 1;
        } else {
            self.current_count = 1;
        }

        self.last_position = position;
        self.last_time = time;
        self.current_count
    }
}

impl WebWindowInner {
    pub fn register_event_listeners(self: &Rc<Self>) -> WebEventListeners {
        let mut closures = self.register_touch_pinch();
        closures.extend(vec![
            self.register_pointer_down(),
            self.register_pointer_up(),
            self.register_pointer_move(),
            self.register_pointer_leave(),
            self.register_wheel(),
            self.register_context_menu(),
            self.register_dragover(),
            self.register_drop(),
            self.register_dragleave(),
            self.register_key_down(),
            self.register_key_up(),
            self.register_paste(),
            self.register_composition_start(),
            self.register_composition_update(),
            self.register_composition_end(),
            self.register_focus(),
            self.register_blur(),
            self.register_pointer_enter(),
            self.register_pointer_leave_hover(),
        ]);
        closures.extend(self.register_visibility_change());
        closures.extend(self.register_appearance_change());

        WebEventListeners { closures }
    }

    fn listen(
        self: &Rc<Self>,
        event_name: &str,
        handler: impl FnMut(JsValue) + 'static,
    ) -> Closure<dyn FnMut(JsValue)> {
        let closure = Closure::<dyn FnMut(JsValue)>::new(handler);
        self.canvas
            .add_event_listener_with_callback(event_name, closure.as_ref().unchecked_ref())
            .ok();
        closure
    }

    fn listen_input(
        self: &Rc<Self>,
        event_name: &str,
        handler: impl FnMut(JsValue) + 'static,
    ) -> Closure<dyn FnMut(JsValue)> {
        let closure = Closure::<dyn FnMut(JsValue)>::new(handler);
        self.input_element
            .add_event_listener_with_callback(event_name, closure.as_ref().unchecked_ref())
            .ok();
        closure
    }

    /// Registers a listener with `{passive: false}` so that `preventDefault()` works.
    /// Needed for events like `wheel` which are passive by default in modern browsers.
    fn listen_non_passive(
        self: &Rc<Self>,
        event_name: &str,
        handler: impl FnMut(JsValue) + 'static,
    ) -> Closure<dyn FnMut(JsValue)> {
        let closure = Closure::<dyn FnMut(JsValue)>::new(handler);
        let canvas_js: &JsValue = self.canvas.as_ref();
        let callback_js: &JsValue = closure.as_ref();
        let options = js_sys::Object::new();
        js_sys::Reflect::set(&options, &"passive".into(), &false.into()).ok();
        if let Ok(add_fn_val) = js_sys::Reflect::get(canvas_js, &"addEventListener".into()) {
            if let Ok(add_fn) = add_fn_val.dyn_into::<js_sys::Function>() {
                add_fn
                    .call3(canvas_js, &event_name.into(), callback_js, &options)
                    .ok();
            }
        }
        closure
    }

    fn dispatch_input(&self, input: PlatformInput) -> Option<DispatchEventResult> {
        let mut borrowed = self.callbacks.borrow_mut();
        borrowed.input.as_mut().map(|callback| callback(input))
    }

    fn register_touch_pinch(self: &Rc<Self>) -> Vec<Closure<dyn FnMut(JsValue)>> {
        let touches = Rc::new(RefCell::new(Vec::<(i32, Point<Pixels>)>::new()));
        let pinching = Rc::new(std::cell::Cell::new(false));
        ["pointerdown", "pointermove", "pointerup", "pointercancel"]
            .into_iter()
            .map(|name| {
                let this = Rc::clone(self);
                let touches = Rc::clone(&touches);
                let pinching = Rc::clone(&pinching);
                let closure = Closure::<dyn FnMut(JsValue)>::new(move |event: JsValue| {
                    let event: web_sys::PointerEvent = event.unchecked_into();
                    if event.pointer_type() != "touch" {
                        return;
                    }
                    let mut touches = touches.borrow_mut();
                    let old_distance = touch_distance(&touches);
                    let position = pointer_position_in_element(&event);
                    match name {
                        "pointerdown" => {
                            touches.push((event.pointer_id(), position));
                            this.canvas.set_pointer_capture(event.pointer_id()).ok();
                        }
                        "pointermove" => {
                            if let Some(touch) =
                                touches.iter_mut().find(|t| t.0 == event.pointer_id())
                            {
                                touch.1 = position;
                            }
                        }
                        _ => touches.retain(|t| t.0 != event.pointer_id()),
                    }
                    if touches.len() >= 2 && !pinching.get() {
                        pinching.set(true);
                        if let Some(button) = this.pressed_button.take() {
                            this.dispatch_input(PlatformInput::MouseUp(MouseUpEvent {
                                button,
                                position,
                                modifiers: Modifiers::default(),
                                click_count: 0,
                            }));
                        }
                    }
                    if pinching.get() || name == "pointercancel" {
                        event.prevent_default();
                        event.stop_immediate_propagation();
                        if name == "pointermove" {
                            if let (Some(old), Some(new)) = (old_distance, touch_distance(&touches))
                            {
                                let center = (touches[0].1 + touches[1].1) / 2.0;
                                this.dispatch_input(PlatformInput::ScrollWheel(ScrollWheelEvent {
                                    position: center,
                                    delta: ScrollDelta::Pixels(point(
                                        px(0.0),
                                        px(pinch_scroll_delta(old, new)),
                                    )),
                                    modifiers: Modifiers {
                                        control: true,
                                        ..Modifiers::default()
                                    },
                                    touch_phase: TouchPhase::Moved,
                                }));
                            }
                        }
                        if touches.is_empty() {
                            pinching.set(false);
                            this.pressed_button.set(None);
                        }
                    }
                });
                self.canvas
                    .add_event_listener_with_callback_and_bool(
                        name,
                        closure.as_ref().unchecked_ref(),
                        true,
                    )
                    .ok();
                closure
            })
            .collect()
    }

    fn register_pointer_down(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen("pointerdown", move |event: JsValue| {
            let event: web_sys::PointerEvent = event.unchecked_into();
            event.prevent_default();
            this.input_element.focus().ok();

            let button = dom_mouse_button_to_gpui(event.button());
            let position = pointer_position_in_element(&event);
            let modifiers = modifiers_from_mouse_event(&event, this.is_mac);
            let time = js_sys::Date::now();

            this.pressed_button.set(Some(button));
            let click_count = this.click_state.borrow_mut().register_click(position, time);

            {
                let mut current_state = this.state.borrow_mut();
                current_state.mouse_position = position;
                current_state.modifiers = modifiers;
            }

            this.dispatch_input(PlatformInput::MouseDown(MouseDownEvent {
                button,
                position,
                modifiers,
                click_count,
                first_mouse: false,
            }));
        })
    }

    fn register_pointer_up(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen("pointerup", move |event: JsValue| {
            let event: web_sys::PointerEvent = event.unchecked_into();
            event.prevent_default();

            let button = dom_mouse_button_to_gpui(event.button());
            let position = pointer_position_in_element(&event);
            let modifiers = modifiers_from_mouse_event(&event, this.is_mac);

            this.pressed_button.set(None);
            let click_count = this.click_state.borrow().current_count;

            {
                let mut current_state = this.state.borrow_mut();
                current_state.mouse_position = position;
                current_state.modifiers = modifiers;
            }

            this.dispatch_input(PlatformInput::MouseUp(MouseUpEvent {
                button,
                position,
                modifiers,
                click_count,
            }));
        })
    }

    fn register_pointer_move(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen("pointermove", move |event: JsValue| {
            let event: web_sys::PointerEvent = event.unchecked_into();
            event.prevent_default();

            let position = pointer_position_in_element(&event);
            let modifiers = modifiers_from_mouse_event(&event, this.is_mac);
            let current_pressed = this.pressed_button.get();

            {
                let mut current_state = this.state.borrow_mut();
                current_state.mouse_position = position;
                current_state.modifiers = modifiers;
            }

            this.dispatch_input(PlatformInput::MouseMove(MouseMoveEvent {
                position,
                pressed_button: current_pressed,
                modifiers,
            }));
        })
    }

    fn register_pointer_leave(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen("pointerleave", move |event: JsValue| {
            let event: web_sys::PointerEvent = event.unchecked_into();

            let position = pointer_position_in_element(&event);
            let modifiers = modifiers_from_mouse_event(&event, this.is_mac);
            let current_pressed = this.pressed_button.get();

            {
                let mut current_state = this.state.borrow_mut();
                current_state.mouse_position = position;
                current_state.modifiers = modifiers;
            }

            this.dispatch_input(PlatformInput::MouseExited(MouseExitEvent {
                position,
                pressed_button: current_pressed,
                modifiers,
            }));
        })
    }

    fn register_wheel(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen_non_passive("wheel", move |event: JsValue| {
            let event: web_sys::WheelEvent = event.unchecked_into();
            event.prevent_default();

            let mouse_event: &web_sys::MouseEvent = event.as_ref();
            let position = mouse_position_in_element(mouse_event);
            let modifiers = modifiers_from_wheel_event(mouse_event, this.is_mac);

            let delta_mode = event.delta_mode();
            let delta = if delta_mode == 1 {
                ScrollDelta::Lines(point(-event.delta_x() as f32, -event.delta_y() as f32))
            } else {
                ScrollDelta::Pixels(point(
                    px(-event.delta_x() as f32),
                    px(-event.delta_y() as f32),
                ))
            };

            {
                let mut current_state = this.state.borrow_mut();
                current_state.modifiers = modifiers;
            }

            let result = this.dispatch_input(PlatformInput::ScrollWheel(ScrollWheelEvent {
                position,
                delta,
                modifiers,
                touch_phase: TouchPhase::Moved,
            }));
            // Let the transcript scroll when the inline visual did not use the wheel.
            if result.is_none_or(|result| result.propagate)
                && this.canvas.has_attribute("data-oa-visual")
                && let Some(source) = this.browser_window.document().and_then(|document| {
                    document
                        .query_selector("canvas:not([data-oa-visual])")
                        .ok()
                        .flatten()
                })
            {
                let init = web_sys::WheelEventInit::new();
                init.set_client_x(event.client_x());
                init.set_client_y(event.client_y());
                init.set_delta_x(event.delta_x());
                init.set_delta_y(event.delta_y());
                init.set_delta_mode(event.delta_mode());
                init.set_ctrl_key(event.ctrl_key());
                init.set_shift_key(event.shift_key());
                init.set_alt_key(event.alt_key());
                init.set_meta_key(event.meta_key());
                if let Ok(forwarded) = web_sys::WheelEvent::new_with_event_init_dict("wheel", &init)
                {
                    source.dispatch_event(&forwarded).ok();
                }
            }
        })
    }

    fn register_context_menu(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        self.listen("contextmenu", move |event: JsValue| {
            let event: web_sys::Event = event.unchecked_into();
            event.prevent_default();
        })
    }

    fn register_dragover(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen("dragover", move |event: JsValue| {
            let event: web_sys::DragEvent = event.unchecked_into();
            event.prevent_default();

            let mouse_event: &web_sys::MouseEvent = event.as_ref();
            let position = mouse_position_in_element(mouse_event);

            {
                let mut current_state = this.state.borrow_mut();
                current_state.mouse_position = position;
            }

            this.dispatch_input(PlatformInput::FileDrop(FileDropEvent::Pending { position }));
        })
    }

    fn register_drop(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen("drop", move |event: JsValue| {
            let event: web_sys::DragEvent = event.unchecked_into();
            event.prevent_default();

            let mouse_event: &web_sys::MouseEvent = event.as_ref();
            let position = mouse_position_in_element(mouse_event);

            {
                let mut current_state = this.state.borrow_mut();
                current_state.mouse_position = position;
            }

            let paths = extract_file_paths_from_drag(&event);

            this.dispatch_input(PlatformInput::FileDrop(FileDropEvent::Entered {
                position,
                paths: ExternalPaths(paths),
            }));

            this.dispatch_input(PlatformInput::FileDrop(FileDropEvent::Submit { position }));
        })
    }

    fn register_dragleave(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen("dragleave", move |_event: JsValue| {
            this.dispatch_input(PlatformInput::FileDrop(FileDropEvent::Exited));
        })
    }

    fn register_key_down(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen_input("keydown", move |event: JsValue| {
            let event: web_sys::KeyboardEvent = event.unchecked_into();

            let modifiers = modifiers_from_keyboard_event(&event, this.is_mac);
            let capslock = capslock_from_keyboard_event(&event);

            {
                let mut current_state = this.state.borrow_mut();
                current_state.modifiers = modifiers;
                current_state.capslock = capslock;
            }

            this.dispatch_input(PlatformInput::ModifiersChanged(ModifiersChangedEvent {
                modifiers,
                capslock,
            }));

            let key = dom_key_to_gpui_key(&event);

            if is_modifier_only_key(&key) {
                return;
            }

            let is_held = event.repeat();
            let key_char = compute_key_char(&event, &key, &modifiers);

            let keystroke = Keystroke {
                modifiers,
                key,
                key_char: key_char.clone(),
            };

            let result = this.dispatch_input(PlatformInput::KeyDown(KeyDownEvent {
                keystroke,
                is_held,
                prefer_character_input: false,
            }));

            if let Some(result) = result {
                if !result.propagate {
                    event.prevent_default();
                    return;
                }
            }

            if this.is_composing.get() || event.is_composing() {
                event.prevent_default();
                return;
            }

            if modifiers.is_subset_of(&Modifiers::shift()) {
                if let Some(text) = key_char {
                    this.with_input_handler(|handler| {
                        handler.replace_text_in_range(None, &text);
                    });
                    // The character went into the input handler; suppress browser
                    // side-effects for the same keystroke (space scrolling the
                    // page, quick-find, etc.). Everything not handled above falls
                    // through so browser shortcuts keep their defaults.
                    event.prevent_default();
                }
            }
        })
    }

    fn register_key_up(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen_input("keyup", move |event: JsValue| {
            let event: web_sys::KeyboardEvent = event.unchecked_into();

            let modifiers = modifiers_from_keyboard_event(&event, this.is_mac);
            let capslock = capslock_from_keyboard_event(&event);

            {
                let mut current_state = this.state.borrow_mut();
                current_state.modifiers = modifiers;
                current_state.capslock = capslock;
            }

            this.dispatch_input(PlatformInput::ModifiersChanged(ModifiersChangedEvent {
                modifiers,
                capslock,
            }));

            let key = dom_key_to_gpui_key(&event);

            if is_modifier_only_key(&key) {
                return;
            }

            let key_char = compute_key_char(&event, &key, &modifiers);

            let keystroke = Keystroke {
                modifiers,
                key,
                key_char,
            };

            let result = this.dispatch_input(PlatformInput::KeyUp(KeyUpEvent { keystroke }));
            if let Some(result) = result {
                if !result.propagate {
                    event.prevent_default();
                }
            }
        })
    }

    /// Paste is delivered through the DOM `paste` event rather than
    /// `Platform::read_from_clipboard`: the browser's asynchronous clipboard
    /// read API cannot fit that synchronous signature, while `ClipboardEvent`
    /// exposes `clipboardData` synchronously inside the event. It fires for
    /// any browser-initiated paste (keyboard, menu bar, context menu).
    fn register_paste(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen_input("paste", move |event: JsValue| {
            let event: web_sys::ClipboardEvent = event.unchecked_into();
            let Some(clipboard_data) = event.clipboard_data() else {
                return;
            };
            let Ok(text) = clipboard_data.get_data("text/plain") else {
                return;
            };
            if text.is_empty() {
                return;
            }

            event.prevent_default();
            this.with_input_handler(|handler| {
                handler.replace_text_in_range(None, &text);
            });
        })
    }

    fn register_composition_start(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen_input("compositionstart", move |_event: JsValue| {
            this.is_composing.set(true);
        })
    }

    fn register_composition_update(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen_input("compositionupdate", move |event: JsValue| {
            let event: web_sys::CompositionEvent = event.unchecked_into();
            let data = event.data().unwrap_or_default();
            this.is_composing.set(true);
            this.with_input_handler(|handler| {
                handler.replace_and_mark_text_in_range(None, &data, None);
            });
        })
    }

    fn register_composition_end(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen_input("compositionend", move |event: JsValue| {
            let event: web_sys::CompositionEvent = event.unchecked_into();
            let data = event.data().unwrap_or_default();
            this.is_composing.set(false);
            this.with_input_handler(|handler| {
                handler.replace_text_in_range(None, &data);
                handler.unmark_text();
            });
            this.input_element.set_value("");
        })
    }

    fn register_focus(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen_input("focus", move |_event: JsValue| {
            {
                let mut state = this.state.borrow_mut();
                state.is_active = true;
            }
            let mut callbacks = this.callbacks.borrow_mut();
            if let Some(ref mut callback) = callbacks.active_status_change {
                callback(true);
            }
        })
    }

    fn register_blur(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen_input("blur", move |_event: JsValue| {
            {
                let mut state = this.state.borrow_mut();
                state.is_active = false;
            }
            let mut callbacks = this.callbacks.borrow_mut();
            if let Some(ref mut callback) = callbacks.active_status_change {
                callback(false);
            }
        })
    }

    fn register_pointer_enter(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen("pointerenter", move |_event: JsValue| {
            {
                let mut state = this.state.borrow_mut();
                state.is_hovered = true;
            }
            let mut callbacks = this.callbacks.borrow_mut();
            if let Some(ref mut callback) = callbacks.hover_status_change {
                callback(true);
            }
        })
    }

    fn register_pointer_leave_hover(self: &Rc<Self>) -> Closure<dyn FnMut(JsValue)> {
        let this = Rc::clone(self);
        self.listen("pointerleave", move |_event: JsValue| {
            {
                let mut state = this.state.borrow_mut();
                state.is_hovered = false;
            }
            let mut callbacks = this.callbacks.borrow_mut();
            if let Some(ref mut callback) = callbacks.hover_status_change {
                callback(false);
            }
        })
    }
}

fn dom_key_to_gpui_key(event: &web_sys::KeyboardEvent) -> String {
    let key = event.key();
    match key.as_str() {
        "Enter" => "enter".to_string(),
        "Backspace" => "backspace".to_string(),
        "Tab" => "tab".to_string(),
        "Escape" => "escape".to_string(),
        "Delete" => "delete".to_string(),
        " " => "space".to_string(),
        "ArrowLeft" => "left".to_string(),
        "ArrowRight" => "right".to_string(),
        "ArrowUp" => "up".to_string(),
        "ArrowDown" => "down".to_string(),
        "Home" => "home".to_string(),
        "End" => "end".to_string(),
        "PageUp" => "pageup".to_string(),
        "PageDown" => "pagedown".to_string(),
        "Insert" => "insert".to_string(),
        "Control" => "control".to_string(),
        "Alt" => "alt".to_string(),
        "Shift" => "shift".to_string(),
        "Meta" => "platform".to_string(),
        "CapsLock" => "capslock".to_string(),
        other => {
            if let Some(rest) = other.strip_prefix('F') {
                if let Ok(number) = rest.parse::<u8>() {
                    if (1..=35).contains(&number) {
                        return format!("f{number}");
                    }
                }
            }
            other.to_lowercase()
        }
    }
}

fn dom_mouse_button_to_gpui(button: i16) -> MouseButton {
    match button {
        0 => MouseButton::Left,
        1 => MouseButton::Middle,
        2 => MouseButton::Right,
        3 => MouseButton::Navigate(NavigationDirection::Back),
        4 => MouseButton::Navigate(NavigationDirection::Forward),
        _ => MouseButton::Left,
    }
}

fn modifiers_from_keyboard_event(event: &web_sys::KeyboardEvent, _is_mac: bool) -> Modifiers {
    Modifiers {
        control: event.ctrl_key(),
        alt: event.alt_key(),
        shift: event.shift_key(),
        platform: event.meta_key(),
        function: false,
    }
}

fn modifiers_from_mouse_event(event: &web_sys::PointerEvent, _is_mac: bool) -> Modifiers {
    let mouse_event: &web_sys::MouseEvent = event.as_ref();
    Modifiers {
        control: mouse_event.ctrl_key(),
        alt: mouse_event.alt_key(),
        shift: mouse_event.shift_key(),
        platform: mouse_event.meta_key(),
        function: false,
    }
}

fn modifiers_from_wheel_event(event: &web_sys::MouseEvent, _is_mac: bool) -> Modifiers {
    Modifiers {
        control: event.ctrl_key(),
        alt: event.alt_key(),
        shift: event.shift_key(),
        platform: event.meta_key(),
        function: false,
    }
}

fn capslock_from_keyboard_event(event: &web_sys::KeyboardEvent) -> Capslock {
    Capslock {
        on: event.get_modifier_state("CapsLock"),
    }
}

pub(crate) fn is_mac_platform(browser_window: &web_sys::Window) -> bool {
    let navigator = browser_window.navigator();

    #[allow(deprecated)]
    // navigator.platform() is deprecated but navigator.userAgentData is not widely available yet
    if let Ok(platform) = navigator.platform() {
        if platform.contains("Mac") {
            return true;
        }
    }

    if let Ok(user_agent) = navigator.user_agent() {
        return user_agent.contains("Mac");
    }

    false
}

fn is_modifier_only_key(key: &str) -> bool {
    matches!(
        key,
        "control" | "alt" | "shift" | "platform" | "capslock" | "compose" | "process"
    )
}

fn compute_key_char(
    event: &web_sys::KeyboardEvent,
    gpui_key: &str,
    modifiers: &Modifiers,
) -> Option<String> {
    if modifiers.platform || modifiers.control {
        return None;
    }

    if is_modifier_only_key(gpui_key) {
        return None;
    }

    if gpui_key == "space" {
        return Some(" ".to_string());
    }

    let raw_key = event.key();

    if raw_key.len() == 1 {
        return Some(raw_key);
    }

    None
}

fn pointer_position_in_element(event: &web_sys::PointerEvent) -> Point<Pixels> {
    let mouse_event: &web_sys::MouseEvent = event.as_ref();
    mouse_position_in_element(mouse_event)
}

fn mouse_position_in_element(event: &web_sys::MouseEvent) -> Point<Pixels> {
    // offset_x/offset_y give position relative to the target element's padding edge
    point(px(event.offset_x() as f32), px(event.offset_y() as f32))
}

fn extract_file_paths_from_drag(
    event: &web_sys::DragEvent,
) -> smallvec::SmallVec<[std::path::PathBuf; 2]> {
    let mut paths = smallvec![];
    let Some(data_transfer) = event.data_transfer() else {
        return paths;
    };
    let file_list = data_transfer.files();
    let Some(files) = file_list else {
        return paths;
    };
    for index in 0..files.length() {
        if let Some(file) = files.get(index) {
            paths.push(std::path::PathBuf::from(file.name()));
        }
    }
    paths
}

fn touch_distance(touches: &[(i32, Point<Pixels>)]) -> Option<f32> {
    let [a, b, ..] = touches else { return None };
    let delta = a.1 - b.1;
    Some(f32::from(delta.x).hypot(f32::from(delta.y)))
}

fn pinch_scroll_delta(old: f32, new: f32) -> f32 {
    if old > 0.0 && new > 0.0 {
        (new / old).ln() / 0.0025
    } else {
        0.0
    }
}

#[cfg(test)]
mod touch_tests {
    #[test]
    fn pinch_scroll_matches_distance_ratio() {
        assert!(((super::pinch_scroll_delta(100.0, 150.0) * 0.0025).exp() - 1.5).abs() < 0.0001);
        assert!(super::pinch_scroll_delta(150.0, 100.0) < 0.0);
        assert_eq!(super::pinch_scroll_delta(0.0, 100.0), 0.0);
    }
}
