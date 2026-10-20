//! A headless GPUI platform for captures at any size and scale.
//!
//! GPUI's own headless window (`TestWindow`) always has a scale of 2 and no
//! subpixel text, so it can only capture a Retina frame in grayscale. This
//! platform's one window takes its scale (a page's device pixel ratio) and
//! whether text is subpixel, and hands each frame it presents to a
//! callback. Tasks run on GPUI's `TestDispatcher`, so a capture is
//! deterministic and needs no run loop.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use anyhow::Result;
use futures::channel::oneshot;
use gpui::{
    Action, AnyWindowHandle, BackgroundExecutor, Bounds, Capslock, ClipboardItem, CursorStyle,
    DispatchEventResult, DummyKeyboardMapper, ForegroundExecutor, GpuSpecs, Keymap, Menu, MenuItem,
    Modifiers, Pixels, Platform, PlatformAtlas, PlatformDisplay, PlatformInput,
    PlatformInputHandler, PlatformKeyboardLayout, PlatformKeyboardMapper, PlatformTextSystem,
    PlatformWindow, Point, PromptButton, PromptLevel, RequestFrameOptions, Scene, Size, Task,
    TestDispatcher, ThermalState, WindowAppearance, WindowBackgroundAppearance, WindowBounds,
    WindowControlArea, WindowParams,
};
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};

/// What a capture window draws with and where its frames go.
#[derive(Clone)]
pub struct Surface {
    pub scale: f32,
    pub subpixel: bool,
    pub atlas: Arc<dyn PlatformAtlas>,
    /// Called with each presented frame and its window's size.
    pub present: Rc<dyn Fn(&Scene, Size<Pixels>, f32)>,
}

pub struct CapturePlatform {
    background: BackgroundExecutor,
    foreground: ForegroundExecutor,
    text: Arc<dyn PlatformTextSystem>,
    surface: Surface,
    /// The open window's frame callback, so the caller can ask it to draw.
    frame: Rc<RefCell<Option<Box<dyn FnMut(RequestFrameOptions)>>>>,
    active: RefCell<Option<AnyWindowHandle>>,
}

impl CapturePlatform {
    pub fn new(
        dispatcher: &TestDispatcher,
        text: Arc<dyn PlatformTextSystem>,
        surface: Surface,
    ) -> Self {
        let dispatcher = Arc::new(dispatcher.clone());
        Self {
            background: BackgroundExecutor::new(dispatcher.clone()),
            foreground: ForegroundExecutor::new(dispatcher),
            text,
            surface,
            frame: Rc::default(),
            active: RefCell::default(),
        }
    }

    /// The window's frame callback, to call outside any app update.
    pub fn frame(&self) -> Rc<RefCell<Option<Box<dyn FnMut(RequestFrameOptions)>>>> {
        self.frame.clone()
    }
}

struct Layout;

impl PlatformKeyboardLayout for Layout {
    fn id(&self) -> &str {
        "capture"
    }
    fn name(&self) -> &str {
        "capture"
    }
}

impl Platform for CapturePlatform {
    fn background_executor(&self) -> BackgroundExecutor {
        self.background.clone()
    }
    fn foreground_executor(&self) -> ForegroundExecutor {
        self.foreground.clone()
    }
    fn text_system(&self) -> Arc<dyn PlatformTextSystem> {
        self.text.clone()
    }
    fn run(&self, on_finish_launching: Box<dyn 'static + FnOnce()>) {
        on_finish_launching();
    }
    fn quit(&self) {}
    fn restart(&self, _: Option<PathBuf>) {}
    fn activate(&self, _: bool) {}
    fn hide(&self) {}
    fn hide_other_apps(&self) {}
    fn unhide_other_apps(&self) {}
    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        vec![]
    }
    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        None
    }
    fn active_window(&self) -> Option<AnyWindowHandle> {
        *self.active.borrow()
    }
    fn open_window(
        &self,
        handle: AnyWindowHandle,
        params: WindowParams,
    ) -> Result<Box<dyn PlatformWindow>> {
        *self.active.borrow_mut() = Some(handle);
        Ok(Box::new(CaptureWindow {
            bounds: params.bounds,
            surface: self.surface.clone(),
            frame: self.frame.clone(),
            input_handler: None,
        }))
    }
    fn window_appearance(&self) -> WindowAppearance {
        WindowAppearance::Dark
    }
    fn open_url(&self, _: &str) {}
    fn on_open_urls(&self, _: Box<dyn FnMut(Vec<String>)>) {}
    fn register_url_scheme(&self, _: &str) -> Task<Result<()>> {
        Task::ready(Ok(()))
    }
    fn prompt_for_paths(
        &self,
        _: gpui::PathPromptOptions,
    ) -> oneshot::Receiver<Result<Option<Vec<PathBuf>>>> {
        let (tx, rx) = oneshot::channel();
        tx.send(Ok(None)).ok();
        rx
    }
    fn prompt_for_new_path(
        &self,
        _: &Path,
        _: Option<&str>,
    ) -> oneshot::Receiver<Result<Option<PathBuf>>> {
        let (tx, rx) = oneshot::channel();
        tx.send(Ok(None)).ok();
        rx
    }
    fn can_select_mixed_files_and_dirs(&self) -> bool {
        false
    }
    fn reveal_path(&self, _: &Path) {}
    fn open_with_system(&self, _: &Path) {}
    fn on_quit(&self, _: Box<dyn FnMut()>) {}
    fn on_reopen(&self, _: Box<dyn FnMut()>) {}
    fn on_system_wake(&self, _: Box<dyn FnMut()>) {}
    fn set_menus(&self, _: Vec<Menu>, _: &Keymap) {}
    fn set_dock_menu(&self, _: Vec<MenuItem>, _: &Keymap) {}
    fn on_app_menu_action(&self, _: Box<dyn FnMut(&dyn Action)>) {}
    fn on_will_open_app_menu(&self, _: Box<dyn FnMut()>) {}
    fn on_validate_app_menu_command(&self, _: Box<dyn FnMut(&dyn Action) -> bool>) {}
    fn thermal_state(&self) -> ThermalState {
        ThermalState::Nominal
    }
    fn on_thermal_state_change(&self, _: Box<dyn FnMut()>) {}
    fn app_path(&self) -> Result<PathBuf> {
        anyhow::bail!("a capture has no app path")
    }
    fn path_for_auxiliary_executable(&self, _: &str) -> Result<PathBuf> {
        anyhow::bail!("a capture has no executables")
    }
    fn set_cursor_style(&self, _: CursorStyle) {}
    fn hide_cursor_until_mouse_moves(&self) {}
    fn is_cursor_visible(&self) -> bool {
        true
    }
    fn should_auto_hide_scrollbars(&self) -> bool {
        true
    }
    fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        None
    }
    fn write_to_clipboard(&self, _: ClipboardItem) {}
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn read_from_primary(&self) -> Option<ClipboardItem> {
        None
    }
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn write_to_primary(&self, _: ClipboardItem) {}
    #[cfg(target_os = "macos")]
    fn read_from_find_pasteboard(&self) -> Option<ClipboardItem> {
        None
    }
    #[cfg(target_os = "macos")]
    fn write_to_find_pasteboard(&self, _: ClipboardItem) {}
    fn write_credentials(&self, _: &str, _: &str, _: &[u8]) -> Task<Result<()>> {
        Task::ready(Ok(()))
    }
    fn read_credentials(&self, _: &str) -> Task<Result<Option<(String, Vec<u8>)>>> {
        Task::ready(Ok(None))
    }
    fn delete_credentials(&self, _: &str) -> Task<Result<()>> {
        Task::ready(Ok(()))
    }
    fn keyboard_layout(&self) -> Box<dyn PlatformKeyboardLayout> {
        Box::new(Layout)
    }
    fn keyboard_mapper(&self) -> Rc<dyn PlatformKeyboardMapper> {
        Rc::new(DummyKeyboardMapper)
    }
    fn on_keyboard_layout_change(&self, _: Box<dyn FnMut()>) {}
}

struct CaptureWindow {
    bounds: Bounds<Pixels>,
    surface: Surface,
    frame: Rc<RefCell<Option<Box<dyn FnMut(RequestFrameOptions)>>>>,
    input_handler: Option<PlatformInputHandler>,
}

impl HasWindowHandle for CaptureWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        Err(HandleError::NotSupported)
    }
}

impl HasDisplayHandle for CaptureWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Err(HandleError::NotSupported)
    }
}

impl PlatformWindow for CaptureWindow {
    fn bounds(&self) -> Bounds<Pixels> {
        self.bounds
    }
    fn is_maximized(&self) -> bool {
        false
    }
    fn window_bounds(&self) -> WindowBounds {
        WindowBounds::Windowed(self.bounds)
    }
    fn content_size(&self) -> Size<Pixels> {
        self.bounds.size
    }
    fn resize(&mut self, size: Size<Pixels>) {
        self.bounds.size = size;
    }
    fn scale_factor(&self) -> f32 {
        self.surface.scale
    }
    fn appearance(&self) -> WindowAppearance {
        WindowAppearance::Dark
    }
    fn display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        None
    }
    fn mouse_position(&self) -> Point<Pixels> {
        Point::default()
    }
    fn modifiers(&self) -> Modifiers {
        Modifiers::default()
    }
    fn capslock(&self) -> Capslock {
        Capslock::default()
    }
    fn set_input_handler(&mut self, input_handler: PlatformInputHandler) {
        self.input_handler = Some(input_handler);
    }
    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        self.input_handler.take()
    }
    fn prompt(
        &self,
        _: PromptLevel,
        _: &str,
        _: Option<&str>,
        _: &[PromptButton],
    ) -> Option<oneshot::Receiver<usize>> {
        None
    }
    fn activate(&self) {}
    fn is_active(&self) -> bool {
        true
    }
    fn is_hovered(&self) -> bool {
        false
    }
    fn background_appearance(&self) -> WindowBackgroundAppearance {
        WindowBackgroundAppearance::Opaque
    }
    fn set_title(&mut self, _: &str) {}
    fn set_background_appearance(&self, _: WindowBackgroundAppearance) {}
    fn minimize(&self) {}
    fn zoom(&self) {}
    fn toggle_fullscreen(&self) {}
    fn is_fullscreen(&self) -> bool {
        false
    }
    fn on_request_frame(&self, callback: Box<dyn FnMut(RequestFrameOptions)>) {
        *self.frame.borrow_mut() = Some(callback);
    }
    fn on_input(&self, _: Box<dyn FnMut(PlatformInput) -> DispatchEventResult>) {}
    fn on_active_status_change(&self, _: Box<dyn FnMut(bool)>) {}
    fn on_hover_status_change(&self, _: Box<dyn FnMut(bool)>) {}
    fn on_resize(&self, _: Box<dyn FnMut(Size<Pixels>, f32)>) {}
    fn on_moved(&self, _: Box<dyn FnMut()>) {}
    fn on_should_close(&self, _: Box<dyn FnMut() -> bool>) {}
    fn on_hit_test_window_control(&self, _: Box<dyn FnMut() -> Option<WindowControlArea>>) {}
    fn on_close(&self, _: Box<dyn FnOnce()>) {}
    fn on_appearance_changed(&self, _: Box<dyn FnMut()>) {}
    fn draw(&self, scene: &Scene) {
        (self.surface.present)(scene, self.bounds.size, self.surface.scale);
    }
    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.surface.atlas.clone()
    }
    fn is_subpixel_rendering_supported(&self) -> bool {
        self.surface.subpixel
    }
    fn gpu_specs(&self) -> Option<GpuSpecs> {
        None
    }
    fn update_ime_position(&self, _: Bounds<Pixels>) {}
    #[cfg(target_os = "windows")]
    fn get_raw_handle(&self) -> windows::Win32::Foundation::HWND {
        unimplemented!()
    }
}
