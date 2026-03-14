use wgpui::{
    Bounds, Component, PaintContext, Point, Quad, RiveFitMode, RiveHandle, RiveSurface, theme,
};

use crate::app_state::{PaneLoadState, PresentationPaneState, PresentationRuntimeState};
use crate::rive_assets::simple_fui_hud_asset;

pub fn paint(
    content_bounds: Bounds,
    pane_state: &mut PresentationPaneState,
    runtime: &mut PresentationRuntimeState,
    paint: &mut PaintContext,
) {
    paint
        .scene
        .draw_quad(Quad::new(content_bounds).with_background(theme::bg::APP));

    ensure_runtime_loaded(pane_state, runtime);
    sync_runtime_state(runtime);

    if let Some(surface) = runtime.surface.as_mut() {
        surface.paint(content_bounds, paint);
        pane_state.load_state = PaneLoadState::Ready;
        pane_state.last_error = None;
    } else if let Some(error) = pane_state.last_error.as_deref() {
        paint.scene.draw_text(paint.text.layout(
            error,
            Point::new(
                content_bounds.origin.x + 24.0,
                content_bounds.origin.y + 24.0,
            ),
            12.0,
            theme::status::ERROR,
        ));
    }
}

fn ensure_runtime_loaded(
    pane_state: &mut PresentationPaneState,
    runtime: &mut PresentationRuntimeState,
) {
    if runtime.surface.is_some() {
        return;
    }

    let asset = simple_fui_hud_asset();
    match RiveSurface::from_bytes_with_handles(
        asset.bytes,
        RiveHandle::Default,
        RiveHandle::Default,
        None,
    ) {
        Ok(mut surface) => {
            sync_controller_state(
                &mut surface,
                &mut runtime.last_applied_fit_mode,
                &mut runtime.last_applied_playing,
                RiveFitMode::Contain,
                true,
            );
            pane_state.asset_id = asset.id.to_string();
            pane_state.asset_name = asset.file_name.to_string();
            pane_state.load_state = PaneLoadState::Ready;
            pane_state.last_error = None;
            pane_state.last_action =
                Some("Presentation surface loaded packaged simple FUI HUD asset".to_string());
            runtime.surface = Some(surface);
        }
        Err(error) => {
            pane_state.load_state = PaneLoadState::Error;
            pane_state.last_error = Some(error.to_string());
            pane_state.last_action = Some("Presentation surface failed to load HUD asset".into());
            runtime.surface = None;
        }
    }
}

fn sync_runtime_state(runtime: &mut PresentationRuntimeState) {
    let Some(surface) = runtime.surface.as_mut() else {
        return;
    };
    let _ = sync_controller_state(
        surface,
        &mut runtime.last_applied_fit_mode,
        &mut runtime.last_applied_playing,
        RiveFitMode::Contain,
        true,
    );
}

fn sync_controller_state(
    surface: &mut RiveSurface,
    last_applied_fit_mode: &mut Option<RiveFitMode>,
    last_applied_playing: &mut Option<bool>,
    desired_fit_mode: RiveFitMode,
    desired_playing: bool,
) -> bool {
    let mut changed = false;
    if *last_applied_fit_mode != Some(desired_fit_mode) {
        surface.controller_mut().set_fit_mode(desired_fit_mode);
        *last_applied_fit_mode = Some(desired_fit_mode);
        changed = true;
    }
    if *last_applied_playing != Some(desired_playing) {
        if desired_playing {
            surface.controller_mut().play();
        } else {
            surface.controller_mut().pause();
        }
        *last_applied_playing = Some(desired_playing);
        changed = true;
    }
    if changed {
        surface.mark_dirty();
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::{paint, sync_runtime_state};
    use crate::app_state::{PaneLoadState, PresentationPaneState, PresentationRuntimeState};
    use wgpui::{Bounds, PaintContext, RiveFitMode, Scene, TextSystem};

    #[test]
    fn presentation_paint_loads_packaged_hud_surface() {
        let mut pane_state = PresentationPaneState::default();
        let mut runtime = PresentationRuntimeState::default();
        let mut scene = Scene::new();
        let mut text_system = TextSystem::new(1.0);
        let mut paint_context = PaintContext::new(&mut scene, &mut text_system, 1.0);

        paint(
            Bounds::new(0.0, 0.0, 960.0, 540.0),
            &mut pane_state,
            &mut runtime,
            &mut paint_context,
        );

        assert!(runtime.surface.is_some());
        assert_eq!(pane_state.load_state, PaneLoadState::Ready);
        assert_eq!(
            runtime
                .surface
                .as_ref()
                .expect("presentation surface")
                .controller()
                .fit_mode(),
            RiveFitMode::Contain
        );
        assert_eq!(runtime.last_applied_fit_mode, Some(RiveFitMode::Contain));
        assert_eq!(runtime.last_applied_playing, Some(true));
    }

    #[test]
    fn presentation_sync_runtime_state_noops_when_controller_state_is_current() {
        let mut pane_state = PresentationPaneState::default();
        let mut runtime = PresentationRuntimeState::default();
        let mut scene = Scene::new();
        let mut text_system = TextSystem::new(1.0);
        let mut paint_context = PaintContext::new(&mut scene, &mut text_system, 1.0);

        paint(
            Bounds::new(0.0, 0.0, 960.0, 540.0),
            &mut pane_state,
            &mut runtime,
            &mut paint_context,
        );

        let before_fit = runtime.last_applied_fit_mode;
        let before_playing = runtime.last_applied_playing;
        sync_runtime_state(&mut runtime);

        assert_eq!(runtime.last_applied_fit_mode, before_fit);
        assert_eq!(runtime.last_applied_playing, before_playing);
    }
}
