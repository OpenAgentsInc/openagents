//! Native SDK host for the bounded Effect Native renderer-adoption spike.
//!
//! The left rail and top bar are Native SDK retained-canvas components. The
//! right pane is a child system WebView loading the real Effect Native DOM
//! renderer from `frontend/dist`. This is intentionally a hybrid proof: Effect
//! does not run inside Native SDK's restricted TypeScript core compiler.

const std = @import("std");
const runner = @import("runner");
const native_sdk = @import("native_sdk");

pub const panic = std.debug.FullPanic(native_sdk.debug.capturePanic);

const canvas = native_sdk.canvas;
const geometry = native_sdk.geometry;

pub const canvas_label = "native-shell";
pub const webview_label = "effect-native-surface";
pub const pane_anchor = "effect-native-pane";
pub const effect_native_url = "zero://app/index.html";

const window_width: f32 = 1120;
const window_height: f32 = 720;
const rail_width: f32 = 304;
const toolbar_height: f32 = 60;

pub const shell_views = [_]native_sdk.ShellView{
    .{
        .label = canvas_label,
        .kind = .gpu_surface,
        .fill = true,
        .role = "Native SDK shell",
        .accessibility_label = "Native SDK component shell",
        .gpu_backend = .metal,
        .gpu_pixel_format = .bgra8_unorm,
        .gpu_present_mode = .timer,
        .gpu_alpha_mode = .@"opaque",
        .gpu_color_space = .srgb,
        .gpu_vsync = true,
    },
    .{
        .label = webview_label,
        .kind = .webview,
        .parent = canvas_label,
        .url = effect_native_url,
        .x = rail_width,
        .y = toolbar_height,
        .width = window_width - rail_width,
        .height = window_height - toolbar_height,
        .layer = 20,
    },
};

pub const shell_windows = [_]native_sdk.ShellWindow{.{
    .label = "main",
    .title = "Native SDK × Effect Native",
    .width = window_width,
    .height = window_height,
    .restore_state = false,
    .views = &shell_views,
}};

pub const shell_scene: native_sdk.ShellConfig = .{ .windows = &shell_windows };

pub const Model = struct {
    frontend_url: []const u8 = effect_native_url,
    native_count: i64 = 0,
    reload_token: u64 = 0,
    details_visible: bool = true,
    gpu_frames_seen: bool = false,
};

pub const Msg = union(enum) {
    increment_native,
    reset_native,
    reload_effect_surface,
    toggle_details,
    frame_presented,
};

pub fn update(model: *Model, msg: Msg) void {
    switch (msg) {
        .increment_native => model.native_count += 1,
        .reset_native => model.native_count = 0,
        .reload_effect_surface => model.reload_token += 1,
        .toggle_details => model.details_visible = !model.details_visible,
        .frame_presented => model.gpu_frames_seen = true,
    }
}

pub const AppUi = canvas.Ui(Msg);
pub const SpikeApp = native_sdk.UiApp(Model, Msg);

fn componentLeaf(ui: *AppUi, kind: canvas.WidgetKind, element_options: AppUi.ElementOptions, label: []const u8) AppUi.Node {
    var node = ui.el(kind, element_options, .{});
    node.widget.text = label;
    return node;
}

fn nativeRail(ui: *AppUi, model: *const Model) AppUi.Node {
    return ui.column(.{
        .width = rail_width,
        .grow = 0,
        .padding = 16,
        .gap = 14,
        .style_tokens = .{ .background = .surface },
        .semantics = .{ .label = "Native SDK component rail" },
    }, .{
        ui.row(.{ .gap = 8, .cross = .center }, .{
            componentLeaf(ui, .badge, .{ .variant = .primary, .size = .sm }, "NATIVE"),
            ui.text(.{ .size = .sm, .style_tokens = .{ .foreground = .text_muted } }, "retained canvas"),
        }),
        ui.el(.card, .{ .padding = 14 }, ui.column(.{ .gap = 10 }, .{
            ui.text(.{ .size = .sm, .style_tokens = .{ .foreground = .text_muted } }, "Typed native update loop"),
            ui.text(.{ .size = .lg }, ui.fmt("{d}", .{model.native_count})),
            ui.row(.{ .gap = 8 }, .{
                ui.button(.{ .variant = .primary, .on_press = .increment_native }, "Increment native"),
                ui.button(.{ .variant = .secondary, .on_press = .reset_native }, "Reset"),
            }),
        })),
        ui.el(.card, .{ .padding = 14 }, ui.column(.{ .gap = 8 }, .{
            ui.text(.{ .size = .sm, .style_tokens = .{ .foreground = .text_muted } }, "Opinionated component kernel"),
            componentLeaf(ui, .badge, .{ .variant = .secondary, .size = .sm }, ui.fmt("{d} built-ins", .{canvas.builtinComponentCount()})),
            componentLeaf(ui, .switch_control, .{
                .checked = model.details_visible,
                .value = if (model.details_visible) 1 else 0,
                .on_toggle = .toggle_details,
                .semantics = .{ .label = "Show component adoption details" },
            }, "Show adoption details"),
            if (model.details_visible)
                ui.column(.{ .gap = 5 }, .{
                    ui.text(.{ .size = .sm }, "Direct: Stack, Text, Button, Card"),
                    ui.text(.{ .size = .sm }, "Composite: List, Table, Split, Select"),
                    ui.text(.{ .size = .sm }, "Host-only: WebView, Chart"),
                })
            else
                ui.text(.{ .size = .sm, .style_tokens = .{ .foreground = .text_muted } }, "Details hidden"),
        })),
        ui.spacer(1),
        ui.statusBar(.{}, if (model.gpu_frames_seen) "native canvas + Effect pane live" else "waiting for first native frame"),
    });
}

pub fn view(ui: *AppUi, model: *const Model) AppUi.Node {
    return ui.column(.{ .gap = 0, .style_tokens = .{ .background = .background } }, .{
        ui.row(.{
            .height = toolbar_height,
            .padding = 12,
            .gap = 10,
            .cross = .center,
            .window_drag = true,
        }, .{
            ui.text(.{ .size = .lg }, "Native SDK × Effect Native"),
            componentLeaf(ui, .badge, .{ .variant = .secondary, .size = .sm }, "HYBRID SPIKE"),
            ui.spacer(1),
            ui.button(.{
                .variant = .secondary,
                .on_press = .reload_effect_surface,
                .semantics = .{ .label = "Reload Effect Native renderer surface" },
            }, "Reload Effect pane"),
        }),
        ui.row(.{ .grow = 1, .gap = 0 }, .{
            nativeRail(ui, model),
            ui.panel(.{
                .grow = 1,
                .semantics = .{ .label = pane_anchor },
            }, .{}),
        }),
    });
}

pub fn panes(model: *const Model, out: []SpikeApp.WebViewPane) usize {
    if (out.len == 0) return 0;
    out[0] = .{
        .label = webview_label,
        .anchor = pane_anchor,
        .url = model.frontend_url,
        .reload_token = model.reload_token,
    };
    return 1;
}

fn onFrame(model: *const Model, frame: native_sdk.platform.GpuFrame) ?Msg {
    _ = frame;
    if (model.gpu_frames_seen) return null;
    return .frame_presented;
}

pub fn options() SpikeApp.Options {
    return .{
        .name = "native-sdk-effect-native-spike",
        .scene = shell_scene,
        .canvas_label = canvas_label,
        .update = update,
        .view = view,
        .web_panes = panes,
        .on_frame = onFrame,
    };
}

pub fn initialModel() Model {
    return .{};
}

pub fn main(init: std.process.Init) !void {
    var model = initialModel();
    if (init.environ_map.get("NATIVE_SDK_FRONTEND_URL")) |url| {
        if (url.len > 0) model.frontend_url = url;
    }
    const app_state = try std.heap.page_allocator.create(SpikeApp);
    defer std.heap.page_allocator.destroy(app_state);
    app_state.* = SpikeApp.init(std.heap.page_allocator, model, options());
    defer app_state.deinit();

    try runner.runWithOptions(app_state.app(), .{
        .app_name = "native-sdk-effect-native-spike",
        .window_title = "Native SDK × Effect Native",
        .bundle_id = "com.openagents.native-sdk-effect-native-spike",
        .icon_path = "assets/icon.png",
        .default_frame = geometry.RectF.init(0, 0, window_width, window_height),
        .restore_state = false,
        .js_window_api = false,
        .security = .{
            .navigation = .{ .allowed_origins = &.{ "zero://app", "http://127.0.0.1:5173" } },
        },
    }, init);
}

test {
    _ = @import("tests.zig");
}
