//! `wlr-layer-shell`: the surfaces anchored to an edge of the screen.
//!
//! A notification daemon, a panel, and a future Coder heads-up display draw
//! on a layer rather than as a window. Smithay's layer map holds them,
//! works out where each anchor and margin puts one, and reports the area
//! the exclusive zones leave, which is the area the tiling layout gets.
//!
//! Each screen has a layer map of its own. The compositor keeps the maps
//! and the layout in step: a layer surface that arrives, changes its zone,
//! or goes away re-arranges the tiles on its screen.

use smithay::desktop::{LayerSurface, WindowSurfaceType, layer_map_for_output};
use smithay::output::Output;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point};
use smithay::wayland::compositor::with_states;
use smithay::wayland::shell::wlr_layer::{
    Layer, LayerSurface as WlrLayerSurface, LayerSurfaceData, WlrLayerShellHandler,
    WlrLayerShellState,
};

use crate::layout::Placed;
use crate::state::Coder;

impl Coder {
    /// Re-reads every screen's layer map and lays the tiles out in what
    /// each one leaves.
    ///
    /// Smithay's `arrange` places every layer surface and configures the
    /// ones whose place changed, and its non-exclusive zone is the screen
    /// less every zone a layer surface reserved.
    pub fn refresh_layers(&mut self) {
        let mut changed = false;
        for output in self.outputs.clone() {
            let zone = {
                let mut map = layer_map_for_output(&output);
                map.arrange();
                let zone = map.non_exclusive_zone();
                Placed {
                    x: zone.loc.x,
                    y: zone.loc.y,
                    width: zone.size.w,
                    height: zone.size.h,
                }
            };
            changed |= self.screens.set_usable(&output.name(), zone);
        }
        if changed {
            self.after_layout();
        }
    }

    /// The screen a layer surface is on.
    fn output_of_layer(&self, surface: &WlSurface) -> Option<Output> {
        self.outputs
            .iter()
            .find(|output| {
                layer_map_for_output(output)
                    .layer_for_surface(surface, WindowSurfaceType::ALL)
                    .is_some()
            })
            .cloned()
    }

    /// Sends a layer surface its first configure, which is what a client
    /// waits for before it draws.
    pub fn configure_new_layer(&mut self, surface: &WlSurface) {
        let Some(output) = self.output_of_layer(surface) else {
            return;
        };
        let configure = {
            let map = layer_map_for_output(&output);
            let Some(layer) = map.layer_for_surface(surface, WindowSurfaceType::TOPLEVEL) else {
                return;
            };
            let sent = with_states(surface, |states| {
                states
                    .data_map
                    .get::<LayerSurfaceData>()
                    .and_then(|data| data.lock().ok().map(|state| state.initial_configure_sent))
                    .unwrap_or(true)
            });
            if sent {
                None
            } else {
                Some(layer.layer_surface().clone())
            }
        };
        if let Some(layer) = configure {
            layer.send_configure();
        }
    }

    /// Whether one surface draws on a layer rather than as a window.
    pub fn is_layer_surface(&self, surface: &WlSurface) -> bool {
        self.output_of_layer(surface).is_some()
    }

    /// The layer surface under the pointer, and where it sits in the shared
    /// space, searching the layers in the order they are given.
    pub fn layer_under(&self, layers: &[Layer]) -> Option<(WlSurface, Point<f64, Logical>)> {
        let output = self
            .screens
            .head_at(self.pointer_at.x, self.pointer_at.y)
            .and_then(|index| self.screens.at(index))
            .and_then(|head| self.output_named(&head.name))?;
        let origin = self.space.output_geometry(output)?.loc;
        let local = self.pointer_at - origin.to_f64();
        let map = layer_map_for_output(output);
        for layer in layers {
            let Some(found) = map.layer_under(*layer, local) else {
                continue;
            };
            let at = map
                .layer_geometry(found)
                .map(|geo| geo.loc)
                .unwrap_or_default();
            let (surface, _) = found.surface_under(local - at.to_f64(), WindowSurfaceType::ALL)?;
            return Some((surface, (origin + at).to_f64()));
        }
        None
    }

    /// Every layer surface one screen holds, top layer last, which is the
    /// order the frame callbacks go out in.
    pub fn layer_surfaces(&self, output: &Output) -> Vec<LayerSurface> {
        let map = layer_map_for_output(output);
        map.layers().cloned().collect()
    }
}

impl WlrLayerShellHandler for Coder {
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.layer_shell_state
    }

    fn new_layer_surface(
        &mut self,
        surface: WlrLayerSurface,
        output: Option<WlOutput>,
        _layer: Layer,
        namespace: String,
    ) {
        // A surface that names a screen goes on it, and one that names none
        // goes on the screen that has the focus.
        let named = output.as_ref().and_then(Output::from_resource);
        let Some(output) = named.or_else(|| self.focused_output().cloned()) else {
            log::warn!("the layer surface {namespace} arrived with no screen to draw on");
            surface.send_close();
            return;
        };
        let layer = LayerSurface::new(surface, namespace.clone());
        {
            let mut map = layer_map_for_output(&output);
            if let Err(err) = map.map_layer(&layer) {
                log::warn!("a layer surface was not mapped: {err}");
                return;
            }
        }
        self.refresh_layers();
        log::debug!(
            "the layer surface {namespace} is on the screen {}",
            output.name()
        );
    }

    fn layer_destroyed(&mut self, surface: WlrLayerSurface) {
        for output in self.outputs.clone() {
            let mut map = layer_map_for_output(&output);
            let found = map
                .layers()
                .find(|layer| layer.layer_surface() == &surface)
                .cloned();
            if let Some(layer) = found {
                map.unmap_layer(&layer);
            }
        }
        self.refresh_layers();
    }
}

smithay::delegate_layer_shell!(Coder);
