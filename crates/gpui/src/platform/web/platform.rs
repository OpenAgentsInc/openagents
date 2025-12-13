//! WebPlatform implementation

use crate::{
    platform::{
        Platform, PlatformDisplay, PlatformTextSystem, ScreenCaptureSource, WindowParams,
    },
    Action, AnyWindowHandle, BackgroundExecutor, ClipboardItem, CursorStyle, DisplayId,
    ForegroundExecutor, Keymap, Menu, MenuItem, PathPromptOptions, PlatformWindow, Task,
    WindowAppearance,
};
use anyhow::Result;
use futures::channel::oneshot;
use std::{
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

use super::dispatcher::WebDispatcher;
use super::text_system::WebTextSystem;
use super::window::WebWindow;

/// Web platform implementation
pub struct WebPlatform {
    dispatcher: Arc<WebDispatcher>,
    text_system: Arc<WebTextSystem>,
}

impl WebPlatform {
    pub fn new() -> Self {
        Self {
            dispatcher: Arc::new(WebDispatcher::new()),
            text_system: Arc::new(WebTextSystem::new()),
        }
    }
}

impl Default for WebPlatform {
    fn default() -> Self {
        Self::new()
    }
}

impl Platform for WebPlatform {
    fn background_executor(&self) -> BackgroundExecutor {
        BackgroundExecutor::new(self.dispatcher.clone())
    }

    fn foreground_executor(&self) -> ForegroundExecutor {
        ForegroundExecutor::new(self.dispatcher.clone())
    }

    fn text_system(&self) -> Arc<dyn PlatformTextSystem> {
        self.text_system.clone()
    }

    fn run(&self, on_finish_launching: Box<dyn 'static + FnOnce()>) {
        on_finish_launching();
        // Web uses requestAnimationFrame loop instead of blocking run
    }

    fn quit(&self) {
        log::info!("quit() called - no-op on web");
    }

    fn restart(&self, _binary_path: Option<PathBuf>) {
        if let Some(window) = web_sys::window() {
            let _ = window.location().reload();
        }
    }

    fn activate(&self, _ignoring_other_apps: bool) {
        // No-op on web
    }

    fn hide(&self) {
        // No-op on web
    }

    fn hide_other_apps(&self) {
        // No-op on web
    }

    fn unhide_other_apps(&self) {
        // No-op on web
    }

    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        vec![Rc::new(WebDisplay::new())]
    }

    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        Some(Rc::new(WebDisplay::new()))
    }

    fn active_window(&self) -> Option<AnyWindowHandle> {
        None
    }

    fn open_window(
        &self,
        handle: AnyWindowHandle,
        options: WindowParams,
    ) -> Result<Box<dyn PlatformWindow>> {
        Ok(Box::new(WebWindow::new(handle, options)?))
    }

    fn window_appearance(&self) -> WindowAppearance {
        // Check prefers-color-scheme
        if let Some(window) = web_sys::window() {
            if let Ok(Some(media_query)) = window.match_media("(prefers-color-scheme: dark)") {
                if media_query.matches() {
                    return WindowAppearance::Dark;
                }
            }
        }
        WindowAppearance::Light
    }

    fn open_url(&self, url: &str) {
        if let Some(window) = web_sys::window() {
            let _ = window.open_with_url_and_target(url, "_blank");
        }
    }

    fn on_open_urls(&self, _callback: Box<dyn FnMut(Vec<String>)>) {
        // No-op on web
    }

    fn register_url_scheme(&self, _url: &str) -> Task<Result<()>> {
        Task::ready(Ok(()))
    }

    fn prompt_for_paths(
        &self,
        _options: PathPromptOptions,
    ) -> oneshot::Receiver<Result<Option<Vec<PathBuf>>>> {
        let (tx, rx) = oneshot::channel();
        // TODO: Use File System Access API
        let _ = tx.send(Ok(None));
        rx
    }

    fn prompt_for_new_path(
        &self,
        _directory: &Path,
        _suggested_name: Option<&str>,
    ) -> oneshot::Receiver<Result<Option<PathBuf>>> {
        let (tx, rx) = oneshot::channel();
        // TODO: Use File System Access API
        let _ = tx.send(Ok(None));
        rx
    }

    fn can_select_mixed_files_and_dirs(&self) -> bool {
        false
    }

    fn reveal_path(&self, _path: &Path) {
        // No-op on web
    }

    fn open_with_system(&self, _path: &Path) {
        // No-op on web
    }

    fn on_quit(&self, _callback: Box<dyn FnMut()>) {
        // TODO: Listen for beforeunload
    }

    fn on_reopen(&self, _callback: Box<dyn FnMut()>) {
        // No-op on web
    }

    fn set_menus(&self, _menus: Vec<Menu>, _keymap: &Keymap) {
        // No-op on web
    }

    fn set_dock_menu(&self, _menu: Vec<MenuItem>, _keymap: &Keymap) {
        // No-op on web
    }

    fn on_app_menu_action(&self, _callback: Box<dyn FnMut(&dyn Action)>) {
        // No-op on web
    }

    fn on_will_open_app_menu(&self, _callback: Box<dyn FnMut()>) {
        // No-op on web
    }

    fn on_validate_app_menu_command(&self, _callback: Box<dyn FnMut(&dyn Action) -> bool>) {
        // No-op on web
    }

    fn app_path(&self) -> Result<PathBuf> {
        Ok(PathBuf::from("/"))
    }

    fn path_for_auxiliary_executable(&self, _name: &str) -> Result<PathBuf> {
        anyhow::bail!("Auxiliary executables not supported on web")
    }

    fn set_cursor_style(&self, style: CursorStyle) {
        if let Some(window) = web_sys::window() {
            if let Some(document) = window.document() {
                if let Some(body) = document.body() {
                    let cursor = match style {
                        CursorStyle::Arrow => "default",
                        CursorStyle::IBeam => "text",
                        CursorStyle::Crosshair => "crosshair",
                        CursorStyle::ClosedHand => "grabbing",
                        CursorStyle::OpenHand => "grab",
                        CursorStyle::PointingHand => "pointer",
                        CursorStyle::ResizeLeft => "w-resize",
                        CursorStyle::ResizeRight => "e-resize",
                        CursorStyle::ResizeLeftRight => "ew-resize",
                        CursorStyle::ResizeUp => "n-resize",
                        CursorStyle::ResizeDown => "s-resize",
                        CursorStyle::ResizeUpDown => "ns-resize",
                        CursorStyle::ResizeUpLeftDownRight => "nesw-resize",
                        CursorStyle::ResizeUpRightDownLeft => "nwse-resize",
                        CursorStyle::ResizeColumn => "col-resize",
                        CursorStyle::ResizeRow => "row-resize",
                        CursorStyle::IBeamCursorForVerticalLayout => "vertical-text",
                        CursorStyle::OperationNotAllowed => "not-allowed",
                        CursorStyle::DragLink => "alias",
                        CursorStyle::DragCopy => "copy",
                        CursorStyle::ContextualMenu => "context-menu",
                        CursorStyle::None => "none",
                    };
                    let _ = body.style().set_property("cursor", cursor);
                }
            }
        }
    }

    fn should_auto_hide_scrollbars(&self) -> bool {
        true
    }

    fn write_to_clipboard(&self, item: ClipboardItem) {
        if let Some(text) = item.text() {
            if let Some(window) = web_sys::window() {
                if let Some(navigator) = window.navigator().clipboard() {
                    let _ = navigator.write_text(&text);
                }
            }
        }
    }

    fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        // Clipboard read is async on web, so we can't do it synchronously
        // This would need to be refactored to be async
        None
    }

    fn write_credentials(&self, _url: &str, _username: &str, _password: &[u8]) -> Task<Result<()>> {
        // TODO: Use Web Credentials API
        Task::ready(Err(anyhow::anyhow!("Credentials not supported on web")))
    }

    fn read_credentials(&self, _url: &str) -> Task<Result<Option<(String, Vec<u8>)>>> {
        Task::ready(Ok(None))
    }

    fn delete_credentials(&self, _url: &str) -> Task<Result<()>> {
        Task::ready(Ok(()))
    }

    fn keyboard_layout(&self) -> Box<dyn crate::platform::PlatformKeyboardLayout> {
        Box::new(WebKeyboardLayout)
    }

    fn keyboard_mapper(&self) -> Rc<dyn crate::platform::PlatformKeyboardMapper> {
        Rc::new(WebKeyboardMapper)
    }

    fn on_keyboard_layout_change(&self, _callback: Box<dyn FnMut()>) {
        // TODO: Listen for keyboard layout changes
    }
}

/// Web display implementation
pub struct WebDisplay {
    bounds: crate::Bounds<crate::Pixels>,
}

impl WebDisplay {
    pub fn new() -> Self {
        let (width, height) = if let Some(window) = web_sys::window() {
            (
                window.inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(1024.0) as f32,
                window.inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(768.0) as f32,
            )
        } else {
            (1024.0, 768.0)
        };

        Self {
            bounds: crate::Bounds {
                origin: crate::Point::zero(),
                size: crate::Size {
                    width: crate::Pixels(width),
                    height: crate::Pixels(height),
                },
            },
        }
    }
}

impl Default for WebDisplay {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for WebDisplay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebDisplay")
            .field("bounds", &self.bounds)
            .finish()
    }
}

impl PlatformDisplay for WebDisplay {
    fn id(&self) -> DisplayId {
        DisplayId(0)
    }

    fn uuid(&self) -> Result<uuid::Uuid> {
        Ok(uuid::Uuid::nil())
    }

    fn bounds(&self) -> crate::Bounds<crate::Pixels> {
        self.bounds
    }
}

/// Web keyboard layout
struct WebKeyboardLayout;

impl crate::platform::PlatformKeyboardLayout for WebKeyboardLayout {
    fn name(&self) -> &str {
        "Web"
    }
}

/// Web keyboard mapper
struct WebKeyboardMapper;

impl crate::platform::PlatformKeyboardMapper for WebKeyboardMapper {
    fn map_keycode(&self, event: &crate::platform::Keystroke) -> crate::platform::Keystroke {
        event.clone()
    }
}
