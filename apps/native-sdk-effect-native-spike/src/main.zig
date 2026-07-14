//! Product-shaped Native SDK host for the bounded Effect Native parity spike.
//!
//! Effect owns workspace/session/message state. The Native SDK rail is only a
//! retained-canvas mirror: clicks emit versioned intents and source selection
//! changes only after a bounded projection returns from the Effect WebView.

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
pub const bridge_command = "openagents.spike.projection.v1";
pub const reload_command = "openagents.spike.reload-effect";
pub const bridge_payload_limit: usize = 8 * 1024;

const window_width: f32 = 1200;
const window_height: f32 = 800;
const window_min_width: f32 = 760;
const window_min_height: f32 = 520;
const rail_width: f32 = 232;

pub const shell_views = [_]native_sdk.ShellView{
    .{
        .label = canvas_label,
        .kind = .gpu_surface,
        .fill = true,
        .role = "OpenAgents native session rail",
        .accessibility_label = "OpenAgents native session rail",
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
        .y = 0,
        .width = window_width - rail_width,
        .height = window_height,
        .layer = 20,
    },
};

pub const shell_windows = [_]native_sdk.ShellWindow{.{
    .label = "main",
    .title = "OpenAgents Native parity spike",
    .width = window_width,
    .height = window_height,
    .min_width = window_min_width,
    .min_height = window_min_height,
    .restore_state = false,
    .titlebar = .hidden_inset,
    .views = &shell_views,
}};

pub const shell_scene: native_sdk.ShellConfig = .{ .windows = &shell_windows };

pub const Workspace = enum { chat, home, settings };
pub const Session = enum { none, parity, renderer, audit };
pub const OutboundIntent = enum {
    none,
    new_chat,
    workspace_chat,
    workspace_home,
    workspace_settings,
    session_parity,
    session_renderer,
    session_audit,
};

pub const Projection = struct {
    revision: u64,
    workspace: Workspace,
    session: Session,
    message_count: u32,
    pending: bool,
    status: []const u8,
};

pub const Model = struct {
    frontend_url: []const u8 = effect_native_url,
    workspace: Workspace = .chat,
    session: Session = .parity,
    projection_revision: u64 = 0,
    message_count: u32 = 0,
    pending: bool = false,
    status_storage: [96]u8 = undefined,
    status_len: usize = 0,
    outbound_intent: OutboundIntent = .none,
    outbound_sequence: u64 = 0,
    awaiting_projection: bool = true,
    reload_token: u64 = 0,
    gpu_frames_seen: bool = false,

    pub fn status(self: *const Model) []const u8 {
        if (self.status_len == 0) return "Waiting for Effect projection";
        return self.status_storage[0..self.status_len];
    }
};

pub const Msg = union(enum) {
    request_new_chat,
    request_workspace_chat,
    request_workspace_home,
    request_workspace_settings,
    request_session_parity,
    request_session_renderer,
    request_session_audit,
    sync_projection: Projection,
    reload_effect_surface,
    frame_presented,
};

fn recordIntent(model: *Model, intent: OutboundIntent) void {
    model.outbound_intent = intent;
    model.outbound_sequence += 1;
    model.awaiting_projection = true;
}

pub fn update(model: *Model, msg: Msg) void {
    switch (msg) {
        .request_new_chat => recordIntent(model, .new_chat),
        .request_workspace_chat => recordIntent(model, .workspace_chat),
        .request_workspace_home => recordIntent(model, .workspace_home),
        .request_workspace_settings => recordIntent(model, .workspace_settings),
        .request_session_parity => recordIntent(model, .session_parity),
        .request_session_renderer => recordIntent(model, .session_renderer),
        .request_session_audit => recordIntent(model, .session_audit),
        .sync_projection => |projection| {
            if (projection.revision <= model.projection_revision) return;
            model.projection_revision = projection.revision;
            model.workspace = projection.workspace;
            model.session = projection.session;
            model.message_count = projection.message_count;
            model.pending = projection.pending;
            model.status_len = @min(projection.status.len, model.status_storage.len);
            @memcpy(model.status_storage[0..model.status_len], projection.status[0..model.status_len]);
            model.awaiting_projection = false;
        },
        .reload_effect_surface => {
            model.reload_token += 1;
            model.awaiting_projection = true;
        },
        .frame_presented => model.gpu_frames_seen = true,
    }
}

pub const AppUi = canvas.Ui(Msg);
pub const SpikeApp = native_sdk.UiApp(Model, Msg);

fn listItem(ui: *AppUi, label: []const u8, selected: bool, msg: Msg) AppUi.Node {
    var node = ui.el(.list_item, .{
        .padding = 9,
        .selected = selected,
        .on_press = msg,
        .semantics = .{ .role = .listitem, .label = label, .focusable = true },
    }, .{});
    node.widget.text = label;
    return node;
}

fn nativeRail(ui: *AppUi, model: *const Model) AppUi.Node {
    return ui.column(.{
        .width = rail_width,
        .grow = 0,
        .padding = 12,
        .gap = 8,
        .style_tokens = .{ .background = .surface },
        .semantics = .{ .label = "OpenAgents session rail" },
    }, .{
        ui.row(.{ .height = 38, .gap = 8, .cross = .center, .window_drag = true }, .{
            ui.spacer(28),
            ui.text(.{ .size = .default }, "OpenAgents"),
        }),
        ui.button(.{ .variant = .primary, .on_press = .request_new_chat, .semantics = .{ .label = "New chat" } }, "New chat"),
        listItem(ui, "Chat", model.workspace == .chat, .request_workspace_chat),
        listItem(ui, "Workspace", model.workspace == .home, .request_workspace_home),
        ui.text(.{ .size = .sm, .style_tokens = .{ .foreground = .text_muted } }, "RECENT"),
        listItem(ui, "Native parity pass", model.session == .parity, .request_session_parity),
        listItem(ui, "Renderer boundary", model.session == .renderer, .request_session_renderer),
        listItem(ui, "SDK adoption audit", model.session == .audit, .request_session_audit),
        ui.spacer(1),
        if (model.awaiting_projection)
            ui.text(.{ .size = .sm, .style_tokens = .{ .foreground = .text_muted } }, "Synchronizing Effect state…")
        else
            ui.text(.{ .size = .sm, .style_tokens = .{ .foreground = .text_muted } }, ui.fmt("{d} messages · revision {d}", .{ model.message_count, model.projection_revision })),
        listItem(ui, "Settings", model.workspace == .settings, .request_workspace_settings),
        ui.statusBar(.{}, model.status()),
    });
}

pub fn view(ui: *AppUi, model: *const Model) AppUi.Node {
    return ui.row(.{ .gap = 0, .style_tokens = .{ .background = .background } }, .{
        nativeRail(ui, model),
        ui.panel(.{ .grow = 1, .semantics = .{ .label = pane_anchor } }, .{}),
    });
}

pub fn panes(model: *const Model, out: []SpikeApp.WebViewPane) usize {
    if (out.len == 0) return 0;
    out[0] = .{ .label = webview_label, .anchor = pane_anchor, .url = model.frontend_url, .reload_token = model.reload_token };
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
        .tokens = canvas.DesignTokens.themeWithOverrides(
            .{ .pack = .geist, .color_scheme = .dark },
            canvas.accentOverrides(canvas.Color.rgb8(58, 123, 255)),
        ),
    };
}

pub fn initialModel() Model {
    return .{};
}

fn parseWorkspace(value: []const u8) ?Workspace {
    if (std.mem.eql(u8, value, "chat")) return .chat;
    if (std.mem.eql(u8, value, "home")) return .home;
    if (std.mem.eql(u8, value, "settings")) return .settings;
    return null;
}

fn fieldValue(payload: []const u8, field: []const u8) ?[]const u8 {
    var pattern_buffer: [96]u8 = undefined;
    const pattern = std.fmt.bufPrint(&pattern_buffer, "\"{s}\"", .{field}) catch return null;
    const start = std.mem.indexOf(u8, payload, pattern) orelse return null;
    var index = start + pattern.len;
    while (index < payload.len and std.ascii.isWhitespace(payload[index])) : (index += 1) {}
    if (index >= payload.len or payload[index] != ':') return null;
    index += 1;
    while (index < payload.len and std.ascii.isWhitespace(payload[index])) : (index += 1) {}
    if (index >= payload.len) return null;
    const value_start = index;
    if (payload[index] == '"') {
        index += 1;
        while (index < payload.len and payload[index] != '"') : (index += 1) {
            if (payload[index] == '\\') return null;
        }
        if (index >= payload.len) return null;
        return payload[value_start .. index + 1];
    }
    while (index < payload.len and payload[index] != ',' and payload[index] != '}') : (index += 1) {}
    return std.mem.trim(u8, payload[value_start..index], " \t\r\n");
}

fn stringField(payload: []const u8, field: []const u8) ?[]const u8 {
    const raw = fieldValue(payload, field) orelse return null;
    if (raw.len < 2 or raw[0] != '"' or raw[raw.len - 1] != '"') return null;
    return raw[1 .. raw.len - 1];
}

fn unsignedField(comptime T: type, payload: []const u8, field: []const u8) ?T {
    return std.fmt.parseUnsigned(T, fieldValue(payload, field) orelse return null, 10) catch null;
}

fn boolField(payload: []const u8, field: []const u8) ?bool {
    const raw = fieldValue(payload, field) orelse return null;
    if (std.mem.eql(u8, raw, "true")) return true;
    if (std.mem.eql(u8, raw, "false")) return false;
    return null;
}

fn parseSession(payload: []const u8) ?Session {
    const raw = fieldValue(payload, "selectedSessionRef") orelse return null;
    if (std.mem.eql(u8, raw, "null")) return .none;
    const value = stringField(payload, "selectedSessionRef") orelse return null;
    if (std.mem.eql(u8, value, "session.parity")) return .parity;
    if (std.mem.eql(u8, value, "session.renderer")) return .renderer;
    if (std.mem.eql(u8, value, "session.audit")) return .audit;
    return null;
}

pub fn parseProjection(payload: []const u8) !Projection {
    if (payload.len > bridge_payload_limit) return error.PayloadTooLarge;
    if (unsignedField(u8, payload, "protocol") != 1) return error.InvalidProtocol;
    const revision = unsignedField(u64, payload, "revision") orelse return error.InvalidProjection;
    const message_count = unsignedField(u32, payload, "messageCount") orelse return error.InvalidProjection;
    if (message_count > 10_000) return error.InvalidProjection;
    const pending = boolField(payload, "pending") orelse return error.InvalidProjection;
    const workspace = parseWorkspace(stringField(payload, "workspace") orelse return error.InvalidProjection) orelse return error.InvalidProjection;
    const session = parseSession(payload) orelse return error.InvalidProjection;
    const status = stringField(payload, "status") orelse return error.InvalidProjection;
    if (status.len > 96) return error.InvalidProjection;
    return .{ .revision = revision, .workspace = workspace, .session = session, .message_count = message_count, .pending = pending, .status = status };
}

fn intentDetail(model: *const Model, output: []u8) ![]const u8 {
    const sequence = model.outbound_sequence;
    return switch (model.outbound_intent) {
        .none => error.NoIntent,
        .new_chat => std.fmt.bufPrint(output, "{{\"protocol\":1,\"sequence\":{d},\"intent\":{{\"_tag\":\"NewChatRequested\"}}}}", .{sequence}),
        .workspace_chat => std.fmt.bufPrint(output, "{{\"protocol\":1,\"sequence\":{d},\"intent\":{{\"_tag\":\"WorkspaceSelected\",\"workspace\":\"chat\"}}}}", .{sequence}),
        .workspace_home => std.fmt.bufPrint(output, "{{\"protocol\":1,\"sequence\":{d},\"intent\":{{\"_tag\":\"WorkspaceSelected\",\"workspace\":\"home\"}}}}", .{sequence}),
        .workspace_settings => std.fmt.bufPrint(output, "{{\"protocol\":1,\"sequence\":{d},\"intent\":{{\"_tag\":\"WorkspaceSelected\",\"workspace\":\"settings\"}}}}", .{sequence}),
        .session_parity => std.fmt.bufPrint(output, "{{\"protocol\":1,\"sequence\":{d},\"intent\":{{\"_tag\":\"SessionSelected\",\"sessionRef\":\"session.parity\"}}}}", .{sequence}),
        .session_renderer => std.fmt.bufPrint(output, "{{\"protocol\":1,\"sequence\":{d},\"intent\":{{\"_tag\":\"SessionSelected\",\"sessionRef\":\"session.renderer\"}}}}", .{sequence}),
        .session_audit => std.fmt.bufPrint(output, "{{\"protocol\":1,\"sequence\":{d},\"intent\":{{\"_tag\":\"SessionSelected\",\"sessionRef\":\"session.audit\"}}}}", .{sequence}),
    };
}

const bridge_origins = [_][]const u8{ "zero://app", "http://127.0.0.1:5173" };
const bridge_policies = [_]native_sdk.BridgeCommandPolicy{.{ .name = bridge_command, .origins = &bridge_origins }};

const HybridHost = struct {
    ui: *SpikeApp,
    inner: native_sdk.App,
    env_map: *std.process.Environ.Map,
    runtime: ?*native_sdk.Runtime = null,
    handlers: [1]native_sdk.BridgeHandler = undefined,

    fn init(ui: *SpikeApp, env_map: *std.process.Environ.Map) @This() {
        return .{ .ui = ui, .inner = ui.app(), .env_map = env_map };
    }

    fn app(self: *@This()) native_sdk.App {
        return .{
            .context = self,
            .name = "native-sdk-effect-native-spike",
            .source = native_sdk.frontend.productionSource(.{ .dist = "frontend/dist", .entry = "index.html" }),
            .source_fn = source,
            .scene_fn = scene,
            .start_fn = start,
            .event_fn = event,
            .stop_fn = stop,
        };
    }

    fn source(context: *anyopaque) anyerror!native_sdk.WebViewSource {
        const self: *@This() = @ptrCast(@alignCast(context));
        return native_sdk.frontend.sourceFromEnv(self.env_map, .{
            .dist = "frontend/dist",
            .entry = "index.html",
        });
    }

    fn bridge(self: *@This()) native_sdk.BridgeDispatcher {
        self.handlers = .{.{ .name = bridge_command, .context = self, .invoke_fn = acceptProjection }};
        return .{ .policy = .{ .enabled = true, .commands = &bridge_policies }, .registry = .{ .handlers = &self.handlers } };
    }

    fn scene(context: *anyopaque) anyerror!native_sdk.ShellConfig {
        const self: *@This() = @ptrCast(@alignCast(context));
        return (try self.inner.scene()).?;
    }

    fn start(context: *anyopaque, runtime: *native_sdk.Runtime) anyerror!void {
        const self: *@This() = @ptrCast(@alignCast(context));
        self.runtime = runtime;
        try self.inner.start(runtime);
    }

    fn event(context: *anyopaque, runtime: *native_sdk.Runtime, value: native_sdk.Event) anyerror!void {
        const self: *@This() = @ptrCast(@alignCast(context));
        switch (value) {
            .command => |command| if (std.mem.eql(u8, command.name, reload_command)) {
                try self.ui.dispatch(runtime, if (command.window_id == 0) 1 else command.window_id, .reload_effect_surface);
                return;
            },
            else => {},
        }
        try self.inner.event(runtime, value);
    }

    fn stop(context: *anyopaque, runtime: *native_sdk.Runtime) anyerror!void {
        const self: *@This() = @ptrCast(@alignCast(context));
        try self.inner.stop(runtime);
        self.runtime = null;
    }

    fn acceptProjection(context: *anyopaque, invocation: native_sdk.bridge.Invocation, output: []u8) anyerror![]const u8 {
        const self: *@This() = @ptrCast(@alignCast(context));
        // The app-level asset source can instantiate a primary WebView while
        // the product surface is the named child pane. Only that pane may
        // project Effect state or consume native intents.
        if (!std.mem.eql(u8, invocation.source.webview_label, webview_label)) return error.InvalidProjectionSource;
        const runtime = self.runtime orelse return error.RuntimeUnavailable;
        const projection = try parseProjection(invocation.request.payload);
        const acknowledged = unsignedField(u64, invocation.request.payload, "acknowledgedNativeSequence") orelse return error.InvalidProjection;
        if (acknowledged > self.ui.model.outbound_sequence) return error.InvalidProjection;
        if (projection.revision > self.ui.model.projection_revision) {
            try self.ui.dispatch(runtime, invocation.source.window_id, .{ .sync_projection = projection });
        }
        if (self.ui.model.outbound_sequence > acknowledged) {
            var detail_buffer: [512]u8 = undefined;
            const detail = try intentDetail(&self.ui.model, &detail_buffer);
            return std.fmt.bufPrint(output, "{{\"accepted\":true,\"revision\":{d},\"intent\":{s}}}", .{ self.ui.model.projection_revision, detail });
        }
        return std.fmt.bufPrint(output, "{{\"accepted\":true,\"revision\":{d},\"intent\":null}}", .{self.ui.model.projection_revision});
    }
};

pub fn main(init: std.process.Init) !void {
    var model = initialModel();
    if (init.environ_map.get("NATIVE_SDK_FRONTEND_URL")) |url| if (url.len > 0) {
        model.frontend_url = url;
    };
    const app_state = try std.heap.page_allocator.create(SpikeApp);
    defer std.heap.page_allocator.destroy(app_state);
    app_state.* = SpikeApp.init(std.heap.page_allocator, model, options());
    defer app_state.deinit();
    var host = HybridHost.init(app_state, init.environ_map);

    try runner.runWithOptions(host.app(), .{
        .app_name = "native-sdk-effect-native-spike",
        .window_title = "OpenAgents Native parity spike",
        .bundle_id = "com.openagents.native-sdk-effect-native-spike",
        .icon_path = "assets/icon.png",
        .default_frame = geometry.RectF.init(0, 0, window_width, window_height),
        .restore_state = false,
        .bridge = host.bridge(),
        .js_window_api = false,
        .security = .{
            .navigation = .{ .allowed_origins = &.{ "zero://app", "http://127.0.0.1:5173" } },
        },
    }, init);
}

test {
    _ = @import("tests.zig");
}
