//! Screens arriving, leaving, and changing scale, on either backend.
//!
//! A backend reports a screen as a `wl_output` with a mode. This module
//! announces the output to clients, places it in the space every window
//! shares, gives it a desk through [`crate::screens`], and keeps three
//! things in step with its scale: the `wl_output` scale a client reads,
//! the fractional scale `wp-fractional-scale` sends each surface on it, and
//! the scale the Xwayland client is mapped through.

use smithay::desktop::layer_map_for_output;
use smithay::desktop::utils::send_frames_surface_tree;
use smithay::output::{Mode, Output, Scale};
use smithay::utils::Transform;
use smithay::wayland::compositor::with_states;
use smithay::wayland::fractional_scale::with_fractional_scale;
use smithay::xwayland::XWaylandClientData;

use crate::layout;
use crate::screens::Head;
use crate::state::Coder;
use crate::xwayland;

impl Coder {
    /// Adds a screen: announces its `wl_output`, gives it a desk, and lays
    /// the windows out across the screens.
    pub fn add_output(&mut self, output: Output, mode: Mode, scale: f64) {
        let name = output.name();
        let size = layout::Screen {
            width: mode.size.w,
            height: mode.size.h,
        };
        let index = self.screens.add(&name, size, scale);
        output.change_current_state(
            Some(mode),
            Some(Transform::Normal),
            Some(Scale::Fractional(scale)),
            None,
        );
        output.set_preferred(mode);
        if !self.outputs.iter().any(|held| held.name() == name) {
            self.outputs.push(output);
        }
        let desk = self
            .screens
            .at(index)
            .map(|head| head.desk + 1)
            .unwrap_or(1);
        log::info!(
            "the screen {name} is {}x{} at scale {scale} and shows desk {desk}",
            size.width,
            size.height
        );
        if self.screens.heads().len() == 1 {
            self.manager.switch_workspace(desk - 1);
            self.warp_to_focused_screen();
        }
        self.place_outputs();
    }

    /// Drops a screen whose monitor left. Its desk stays in the layout, and
    /// its windows come back when a screen shows that desk.
    pub fn remove_output(&mut self, name: &str) {
        let Some(index) = self.outputs.iter().position(|held| held.name() == name) else {
            return;
        };
        let output = self.outputs.remove(index);
        {
            let mut map = layer_map_for_output(&output);
            let layers: Vec<_> = map.layers().cloned().collect();
            for layer in layers {
                layer.layer_surface().send_close();
                map.unmap_layer(&layer);
            }
        }
        self.space.unmap_output(&output);
        self.screens.remove(name);
        log::info!("the screen {name} left");
        if let Some(head) = self.screens.focused() {
            self.manager.switch_workspace(head.desk);
        }
        let (x, y) = self.screens.clamp(self.pointer_at.x, self.pointer_at.y);
        self.pointer_at = (x, y).into();
        self.place_outputs();
    }

    /// Changes one screen's mode, which the nested window does when it is
    /// resized.
    pub fn resize_output(&mut self, name: &str, mode: Mode) {
        let Some(output) = self.output_named(name).cloned() else {
            return;
        };
        output.change_current_state(Some(mode), None, None, None);
        output.set_preferred(mode);
        self.screens.set_mode(
            name,
            layout::Screen {
                width: mode.size.w,
                height: mode.size.h,
            },
        );
        self.place_outputs();
    }

    /// Sets the scale one screen draws at, and answers the scale it took
    /// after [`crate::screens::snap_scale`].
    pub fn set_output_scale(&mut self, name: &str, requested: f64) -> Result<f64, String> {
        let Some(output) = self.output_named(name).cloned() else {
            return Err(format!("no screen is named {name}"));
        };
        let scale = self.screens.set_scale(name, requested)?;
        output.change_current_state(None, None, Some(Scale::Fractional(scale)), None);
        log::info!("the screen {name} draws at scale {scale}");
        self.place_outputs();
        Ok(scale)
    }

    /// Moves every output to the place its screen holds in the shared
    /// space, then lays the layers and the windows out on them.
    fn place_outputs(&mut self) {
        let heads: Vec<Head> = self.screens.heads().to_vec();
        for head in &heads {
            if let Some(output) = self.output_named(&head.name).cloned() {
                output.change_current_state(None, None, None, Some(head.at.into()));
                self.space.map_output(&output, head.at);
            }
        }
        self.set_xwayland_scale();
        self.refresh_layers();
        self.after_layout();
    }

    /// Sends each surface the scale of the screen it draws on, through
    /// `wp-fractional-scale`. A surface that sits on no screen yet reads
    /// the focused screen's.
    pub fn send_scales(&self) {
        let fallback = self
            .focused_output()
            .map(|output| output.current_scale().fractional_scale())
            .unwrap_or(1.0);
        for window in self.space.elements() {
            let scale = self
                .space
                .outputs_for_element(window)
                .first()
                .map(|output| output.current_scale().fractional_scale())
                .unwrap_or(fallback);
            window.with_surfaces(|_, states| {
                with_fractional_scale(states, |fractional| {
                    fractional.set_preferred_scale(scale);
                });
            });
        }
        for output in &self.outputs {
            let scale = output.current_scale().fractional_scale();
            let map = layer_map_for_output(output);
            for layer in map.layers() {
                layer.with_surfaces(|_, states| {
                    with_fractional_scale(states, |fractional| {
                        fractional.set_preferred_scale(scale);
                    });
                });
            }
        }
    }

    /// Tells every client drawing on one screen that its frame was drawn,
    /// which is what a client waits for before it draws the next one.
    pub fn send_frames(&self, output: &Output) {
        let time = self.started.elapsed();
        let throttle = Some(std::time::Duration::from_secs(1));
        for window in self.space.elements_for_output(output) {
            window.send_frame(output, time, throttle, |_, _| Some(output.clone()));
        }
        for layer in self.layer_surfaces(output) {
            layer.send_frame(output, time, throttle, |_, _| Some(output.clone()));
        }
        let pointer_here = self
            .space
            .output_geometry(output)
            .is_some_and(|geometry| geometry.to_f64().contains(self.pointer_at));
        if !pointer_here {
            return;
        }
        if let smithay::input::pointer::CursorImageStatus::Surface(surface) = &self.cursor_status {
            send_frames_surface_tree(surface, output, time, throttle, |_, _| Some(output.clone()));
        }
        if let Some(icon) = &self.drag_icon {
            send_frames_surface_tree(icon, output, time, throttle, |_, _| Some(output.clone()));
        }
    }

    /// Sends one new surface the scale of the focused screen, which is
    /// where a window opens.
    pub fn send_first_scale(
        &self,
        surface: &smithay::reexports::wayland_server::protocol::wl_surface::WlSurface,
    ) {
        let scale = self
            .focused_output()
            .map(|output| output.current_scale().fractional_scale())
            .unwrap_or(1.0);
        with_states(surface, |states| {
            with_fractional_scale(states, |fractional| {
                fractional.set_preferred_scale(scale);
            });
        });
    }

    /// Maps the Xwayland client through the largest scale any screen draws
    /// at, which is Hyprland's `xwayland:force_zero_scaling`.
    pub fn set_xwayland_scale(&self) {
        let largest = self
            .screens
            .heads()
            .iter()
            .map(|head| head.scale)
            .fold(1.0_f64, f64::max);
        if let Some(client) = self.xwayland.client.as_ref()
            && let Some(data) = client.get_data::<XWaylandClientData>()
        {
            data.compositor_state
                .set_client_scale(xwayland::client_scale(largest));
        }
    }
}
