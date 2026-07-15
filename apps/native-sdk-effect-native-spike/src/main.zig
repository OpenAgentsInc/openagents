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
pub const effect_native_base_url = "zero://app/index.html";
pub const effect_native_url = effect_native_base_url ++ "#surface=effect-native";
pub const bridge_command = "openagents.spike.projection.v1";
pub const reload_command = "openagents.spike.reload-effect";
pub const chat_new_command = "chat.new";
pub const bridge_payload_limit: usize = 8 * 1024;
pub const sidecar_frame_limit: usize = 64 * 1024;
pub const sidecar_protocol = "openagents.desktop.native-sidecar.v2";
pub const sidecar_rpc_protocol = "openagents.desktop.native-sidecar-rpc.v1";
pub const sidecar_node_version = "24.13.1";
pub const sidecar_gateway_protocol: u8 = 11;
pub const sidecar_effect_key: u64 = 0x4f_41_4e_53;
pub const sidecar_rpc_effect_key: u64 = 0x4f_41_4e_52;

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

pub const file_menu_items = [_]native_sdk.MenuItem{.{
    .label = "New Chat",
    .command = chat_new_command,
    .key = "n",
    .modifiers = .{ .command = true },
}};
pub const app_menus = [_]native_sdk.Menu{.{ .title = "File", .items = &file_menu_items }};

pub const Workspace = enum { chat, home, settings };
pub const Session = enum { none, parity, renderer, audit };
pub const SidecarPhase = enum { unconfigured, starting, ready, unavailable };
pub const RepositoryPhase = enum { empty, choosing, admitting, ready, refused };
pub const OutboundIntent = enum {
    none,
    new_chat,
    new_chat_menu,
    reload_effect,
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
    last_applied_command_sequence: u64,
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
    last_applied_command_sequence: u64 = 0,
    sidecar_phase: SidecarPhase = .unconfigured,
    sidecar_node_path: []const u8 = "",
    sidecar_entry_path: []const u8 = "",
    sidecar_nonce: []const u8 = "native.local",
    sidecar_generation: u64 = 1,
    sidecar_pid: u32 = 0,
    sidecar_gateway_protocol: u8 = 0,
    sidecar_port: u16 = 0,
    sidecar_state_root: []const u8 = "",
    sidecar_token: []const u8 = "",
    repository_phase: RepositoryPhase = .empty,
    repository_dialog_requested: bool = false,
    repository_root_storage: [4096]u8 = undefined,
    repository_root_len: usize = 0,
    repository_request_sequence: u64 = 0,
    grant_ref_storage: [257]u8 = undefined,
    grant_ref_len: usize = 0,
    project_ref_storage: [257]u8 = undefined,
    project_ref_len: usize = 0,
    repository_ref_storage: [257]u8 = undefined,
    repository_ref_len: usize = 0,
    worktree_ref_storage: [257]u8 = undefined,
    worktree_ref_len: usize = 0,
    work_context_ref_storage: [257]u8 = undefined,
    work_context_ref_len: usize = 0,
    session_ref_storage: [257]u8 = undefined,
    session_ref_len: usize = 0,
    catalog_total_sessions: u32 = 0,

    pub fn status(self: *const Model) []const u8 {
        if (self.status_len == 0) return "Waiting for Effect projection";
        return self.status_storage[0..self.status_len];
    }

    pub fn sidecarStatus(self: *const Model, arena: std.mem.Allocator) []const u8 {
        return switch (self.sidecar_phase) {
            .unconfigured => "Desktop runtime gateway unavailable · exact Node sidecar not configured",
            .starting => "Desktop runtime gateway · starting exact Node 24 sidecar…",
            .unavailable => "Desktop runtime gateway unavailable · sidecar bootstrap refused",
            .ready => std.fmt.allocPrint(
                arena,
                "Desktop runtime gateway v{d} · Node {s} · generation {d} · private sidecar ready",
                .{ self.sidecar_gateway_protocol, sidecar_node_version, self.sidecar_generation },
            ) catch "Desktop runtime gateway ready",
        };
    }

    pub fn repositoryStatus(self: *const Model, arena: std.mem.Allocator) []const u8 {
        return switch (self.repository_phase) {
            .empty => "No repository granted",
            .choosing => "Choosing repository…",
            .admitting => "Admitting repository through Desktop authority…",
            .refused => "Repository grant refused",
            .ready => std.fmt.allocPrint(
                arena,
                "Catalog {d} · request {d} · Grant {s} · Project {s} · Repository {s} · Worktree {s} · WorkContext {s} · Session {s}",
                .{
                    self.catalog_total_sessions,
                    self.repository_request_sequence,
                    self.grant_ref_storage[0..self.grant_ref_len],
                    self.project_ref_storage[0..self.project_ref_len],
                    self.repository_ref_storage[0..self.repository_ref_len],
                    self.worktree_ref_storage[0..self.worktree_ref_len],
                    self.work_context_ref_storage[0..self.work_context_ref_len],
                    self.session_ref_storage[0..self.session_ref_len],
                },
            ) catch "Repository ready",
        };
    }
};

pub const Msg = union(enum) {
    request_new_chat,
    request_new_chat_menu,
    request_workspace_chat,
    request_workspace_home,
    request_workspace_settings,
    request_session_parity,
    request_session_renderer,
    request_session_audit,
    sync_projection: Projection,
    reload_effect_surface,
    frame_presented,
    request_repository_dialog,
    request_repository_admit,
    sidecar_ready_line: native_sdk.EffectLine,
    sidecar_exited: native_sdk.EffectExit,
    sidecar_rpc_finished: native_sdk.EffectResponse,
};

fn recordIntent(model: *Model, intent: OutboundIntent) void {
    model.outbound_intent = intent;
    model.outbound_sequence += 1;
    model.awaiting_projection = true;
}

pub fn update(model: *Model, msg: Msg) void {
    switch (msg) {
        .request_new_chat => recordIntent(model, .new_chat),
        .request_new_chat_menu => recordIntent(model, .new_chat_menu),
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
            model.last_applied_command_sequence = projection.last_applied_command_sequence;
            model.status_len = @min(projection.status.len, model.status_storage.len);
            @memcpy(model.status_storage[0..model.status_len], projection.status[0..model.status_len]);
            model.awaiting_projection = false;
        },
        .reload_effect_surface => {
            recordIntent(model, .reload_effect);
        },
        .frame_presented => model.gpu_frames_seen = true,
        .request_repository_dialog => {
            model.repository_dialog_requested = true;
            model.repository_phase = .choosing;
        },
        .request_repository_admit => model.repository_phase = .admitting,
        .sidecar_ready_line => |line| applySidecarReadyLine(model, line),
        .sidecar_exited => |result| applySidecarExit(model, result),
        .sidecar_rpc_finished => |result| applySidecarRpcResult(model, result),
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
        ui.button(.{
            .on_press = .request_repository_dialog,
            .semantics = .{
                .label = "Grant repository",
                .actions = .{ .drop_files = true },
            },
        }, "Grant repository"),
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
        if (model.last_applied_command_sequence > 0)
            ui.text(.{ .size = .sm, .style_tokens = .{ .foreground = .text_muted } }, ui.fmt("Applied chat.new → DesktopNewChat · native_menu · sequence {d}", .{model.last_applied_command_sequence}))
        else
            ui.text(.{ .size = .sm, .style_tokens = .{ .foreground = .text_muted } }, "Native menu command not applied"),
        ui.text(.{ .size = .sm, .style_tokens = .{ .foreground = .text_muted } }, model.sidecarStatus(ui.arena)),
        ui.text(.{ .size = .sm, .style_tokens = .{ .foreground = .text_muted } }, model.repositoryStatus(ui.arena)),
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
        .update_fx = updateWithEffects,
        .init_fx = initEffects,
        .view = view,
        .on_command = command,
        .web_panes = panes,
        .on_frame = onFrame,
        .tokens = canvas.DesignTokens.themeWithOverrides(
            .{ .pack = .geist, .color_scheme = .dark },
            canvas.accentOverrides(canvas.Color.rgb8(58, 123, 255)),
        ),
    };
}

pub const Effects = SpikeApp.Effects;

fn base64Encode(input: []const u8, output: []u8) ![]const u8 {
    const size = std.base64.standard.Encoder.calcSize(input.len);
    if (size > output.len) return error.NoSpaceLeft;
    return std.base64.standard.Encoder.encode(output[0..size], input);
}

pub fn initEffects(model: *Model, fx: *Effects) void {
    if (model.sidecar_node_path.len == 0 or model.sidecar_entry_path.len == 0 or
        model.sidecar_state_root.len == 0 or model.sidecar_token.len != 64)
    {
        model.sidecar_phase = .unconfigured;
        return;
    }
    var state_root_buffer: [4096]u8 = undefined;
    const encoded_state_root = base64Encode(model.sidecar_state_root, &state_root_buffer) catch {
        model.sidecar_phase = .unavailable;
        return;
    };
    var request_buffer: [4096]u8 = undefined;
    const request = std.fmt.bufPrint(
        &request_buffer,
        "{{\"protocol\":\"{s}\",\"generation\":{d},\"nonce\":\"{s}\",\"stateRootBase64\":\"{s}\",\"transportToken\":\"{s}\"}}",
        .{ sidecar_protocol, model.sidecar_generation, model.sidecar_nonce, encoded_state_root, model.sidecar_token },
    ) catch {
        model.sidecar_phase = .unavailable;
        return;
    };
    model.sidecar_phase = .starting;
    fx.spawn(.{
        .key = sidecar_effect_key,
        .argv = &.{ model.sidecar_node_path, model.sidecar_entry_path },
        .stdin = request,
        .output = .lines,
        .max_line_bytes = sidecar_frame_limit,
        .on_line = Effects.lineMsg(.sidecar_ready_line),
        .on_exit = Effects.exitMsg(.sidecar_exited),
    });
}

fn startCodingRequest(model: *Model, fx: *Effects, operation: enum { snapshot, admit }) void {
    if (model.sidecar_phase != .ready or model.sidecar_port == 0 or model.sidecar_token.len != 64) {
        model.repository_phase = .refused;
        return;
    }
    model.repository_request_sequence += 1;
    var url_buffer: [96]u8 = undefined;
    const url = std.fmt.bufPrint(&url_buffer, "http://127.0.0.1:{d}/v1/coding", .{model.sidecar_port}) catch {
        model.repository_phase = .refused;
        return;
    };
    var authorization_buffer: [80]u8 = undefined;
    const authorization = std.fmt.bufPrint(&authorization_buffer, "Bearer {s}", .{model.sidecar_token}) catch {
        model.repository_phase = .refused;
        return;
    };
    var request_buffer: [8192]u8 = undefined;
    const request = switch (operation) {
        .snapshot => std.fmt.bufPrint(
            &request_buffer,
            "{{\"protocol\":\"{s}\",\"generation\":{d},\"nonce\":\"{s}\",\"requestId\":\"native.coding.{d}\",\"operation\":\"coding.snapshot\"}}",
            .{ sidecar_rpc_protocol, model.sidecar_generation, model.sidecar_nonce, model.repository_request_sequence },
        ),
        .admit => blk: {
            if (model.repository_root_len == 0) break :blk error.NoRepositoryRoot;
            var root_buffer: [6144]u8 = undefined;
            const root = base64Encode(model.repository_root_storage[0..model.repository_root_len], &root_buffer) catch break :blk error.NoSpaceLeft;
            break :blk std.fmt.bufPrint(
                &request_buffer,
                "{{\"protocol\":\"{s}\",\"generation\":{d},\"nonce\":\"{s}\",\"requestId\":\"native.coding.{d}\",\"operation\":\"coding.admit\",\"rootBase64\":\"{s}\"}}",
                .{ sidecar_rpc_protocol, model.sidecar_generation, model.sidecar_nonce, model.repository_request_sequence, root },
            );
        },
    } catch {
        model.repository_phase = .refused;
        return;
    };
    const headers = [_]std.http.Header{
        .{ .name = "authorization", .value = authorization },
        .{ .name = "content-type", .value = "application/json" },
    };
    fx.fetch(.{
        .key = sidecar_rpc_effect_key,
        .method = .POST,
        .url = url,
        .headers = &headers,
        .body = request,
        .timeout_ms = 10_000,
        .on_response = Effects.responseMsg(.sidecar_rpc_finished),
    });
}

pub fn updateWithEffects(model: *Model, msg: Msg, fx: *Effects) void {
    const tag = std.meta.activeTag(msg);
    update(model, msg);
    switch (tag) {
        .sidecar_ready_line => if (model.sidecar_phase == .ready)
            startCodingRequest(model, fx, .snapshot)
        else
            fx.cancel(sidecar_effect_key),
        .request_repository_admit => startCodingRequest(model, fx, .admit),
        else => {},
    }
}

pub fn validRunNamespace(value: []const u8) bool {
    if (value.len == 0 or value.len > 80) return false;
    for (value) |character| {
        if (!std.ascii.isAlphanumeric(character) and character != '.' and character != '_' and character != '-') return false;
    }
    return true;
}

pub fn initialModel() Model {
    return .{};
}

pub fn command(name: []const u8) ?Msg {
    if (std.mem.eql(u8, name, chat_new_command)) return .request_new_chat_menu;
    return null;
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
    const applied_raw = fieldValue(payload, "lastAppliedCommand") orelse return error.InvalidProjection;
    var last_applied_command_sequence: u64 = 0;
    if (!std.mem.eql(u8, applied_raw, "null")) {
        last_applied_command_sequence = unsignedField(u64, payload, "sequence") orelse return error.InvalidProjection;
        if (last_applied_command_sequence == 0 or
            !std.mem.eql(u8, stringField(payload, "commandId") orelse return error.InvalidProjection, chat_new_command) or
            !std.mem.eql(u8, stringField(payload, "intentName") orelse return error.InvalidProjection, "DesktopNewChat") or
            !std.mem.eql(u8, stringField(payload, "source") orelse return error.InvalidProjection, "native_menu")) return error.InvalidProjection;
    }
    return .{ .revision = revision, .workspace = workspace, .session = session, .message_count = message_count, .pending = pending, .status = status, .last_applied_command_sequence = last_applied_command_sequence };
}

pub const SidecarBootstrapReceipt = struct {
    generation: u64,
    pid: u32,
    gateway_protocol: u8,
    port: u16,
};

const SidecarBootstrapResultWire = struct {
    kind: []const u8,
    protocolVersion: u8,
    lifecycle: []const u8,
    sessionPhase: []const u8,
    identityTier: []const u8,
    capabilities: []const std.json.Value,
};

const SidecarGatewayResponseWire = struct {
    kind: []const u8,
    requestId: []const u8,
    result: SidecarBootstrapResultWire,
};

const SidecarBootstrapReceiptWire = struct {
    protocol: []const u8,
    generation: u64,
    nonce: []const u8,
    pid: u32,
    nodeVersion: []const u8,
    gatewayProtocolVersion: u8,
    requestId: []const u8,
    response: SidecarGatewayResponseWire,
    transport: struct {
        kind: []const u8,
        host: []const u8,
        port: u16,
    },
};

pub fn parseSidecarBootstrapReceipt(payload: []const u8, expected_generation: u64, expected_nonce: []const u8) !SidecarBootstrapReceipt {
    if (payload.len == 0 or payload.len > sidecar_frame_limit) return error.InvalidSidecarReceipt;
    var parsed = std.json.parseFromSlice(SidecarBootstrapReceiptWire, std.heap.page_allocator, payload, .{}) catch return error.InvalidSidecarReceipt;
    defer parsed.deinit();
    const receipt = parsed.value;
    if (!std.mem.eql(u8, receipt.protocol, sidecar_protocol) or
        receipt.generation != expected_generation or
        !std.mem.eql(u8, receipt.nonce, expected_nonce) or
        receipt.pid == 0 or
        !std.mem.eql(u8, receipt.nodeVersion, sidecar_node_version) or
        receipt.gatewayProtocolVersion != sidecar_gateway_protocol or
        !std.mem.eql(u8, receipt.requestId, "native-sidecar.bootstrap") or
        !std.mem.eql(u8, receipt.response.kind, "query_result") or
        !std.mem.eql(u8, receipt.response.requestId, "native-sidecar.bootstrap") or
        !std.mem.eql(u8, receipt.response.result.kind, "runtime.bootstrap") or
        receipt.response.result.protocolVersion != sidecar_gateway_protocol or
        !std.mem.eql(u8, receipt.response.result.lifecycle, "ready") or
        !std.mem.eql(u8, receipt.transport.kind, "loopback_http") or
        !std.mem.eql(u8, receipt.transport.host, "127.0.0.1") or
        receipt.transport.port < 1024) return error.InvalidSidecarReceipt;
    return .{
        .generation = receipt.generation,
        .pid = receipt.pid,
        .gateway_protocol = receipt.gatewayProtocolVersion,
        .port = receipt.transport.port,
    };
}

fn applySidecarReadyLine(model: *Model, line: native_sdk.EffectLine) void {
    if (line.key != sidecar_effect_key or line.truncated or line.dropped_before != 0 or model.sidecar_phase != .starting) {
        model.sidecar_phase = .unavailable;
        return;
    }
    const receipt = parseSidecarBootstrapReceipt(
        std.mem.trim(u8, line.line, " \t\r\n"),
        model.sidecar_generation,
        model.sidecar_nonce,
    ) catch {
        model.sidecar_phase = .unavailable;
        return;
    };
    model.sidecar_pid = receipt.pid;
    model.sidecar_gateway_protocol = receipt.gateway_protocol;
    model.sidecar_port = receipt.port;
    model.sidecar_phase = .ready;
}

fn applySidecarExit(model: *Model, result: native_sdk.EffectExit) void {
    if (result.key != sidecar_effect_key) return;
    if (result.reason == .cancelled) return;
    model.sidecar_phase = .unavailable;
    model.sidecar_port = 0;
}

const CodingAdmissionWire = struct {
    grantRef: []const u8,
    projectRef: []const u8,
    repositoryRef: []const u8,
    worktreeRef: []const u8,
    workContextRef: []const u8,
    sessionRef: []const u8,
};

const CodingSessionWire = struct {
    sessionRef: []const u8,
    workContextRef: ?[]const u8 = null,
    grantRef: ?[]const u8 = null,
    projectRef: []const u8,
    repositoryRef: []const u8,
    worktreeRef: []const u8,
    projectLabel: []const u8,
    repositoryLabel: []const u8,
    worktreeLabel: []const u8,
    state: []const u8,
    lastActiveAt: []const u8,
    recoveryReason: ?[]const u8,
};

const CodingProjectionWire = struct {
    authority: []const u8,
    authorityLabel: []const u8,
    selectedSessionRef: ?[]const u8,
    focus: std.json.Value,
    sessions: []const CodingSessionWire,
    pageOffset: u32,
    totalSessions: u32,
    nextOffset: ?u32,
    activeCount: u32,
    recoveryCount: u32,
    archivedCount: u32,
};

const CodingRpcResponseWire = struct {
    protocol: []const u8,
    generation: u64,
    nonce: []const u8,
    requestId: []const u8,
    result: struct {
        kind: []const u8,
        reason: ?[]const u8 = null,
        projection: CodingProjectionWire,
        projectionDigest: []const u8,
        admission: ?CodingAdmissionWire = null,
    },
};

fn validCodingRef(value: []const u8) bool {
    if (value.len == 0 or value.len > 256 or !std.ascii.isAlphanumeric(value[0])) return false;
    for (value[1..]) |character| {
        if (!std.ascii.isAlphanumeric(character) and character != '.' and character != '_' and character != ':' and character != '-') return false;
    }
    return true;
}

fn copyRef(storage: []u8, length: *usize, value: []const u8) bool {
    if (!validCodingRef(value) or value.len > storage.len) return false;
    @memcpy(storage[0..value.len], value);
    length.* = value.len;
    return true;
}

fn admissionMatchesSession(admission: CodingAdmissionWire, session: CodingSessionWire) bool {
    return session.grantRef != null and session.workContextRef != null and
        std.mem.eql(u8, admission.grantRef, session.grantRef.?) and
        std.mem.eql(u8, admission.projectRef, session.projectRef) and
        std.mem.eql(u8, admission.repositoryRef, session.repositoryRef) and
        std.mem.eql(u8, admission.worktreeRef, session.worktreeRef) and
        std.mem.eql(u8, admission.workContextRef, session.workContextRef.?) and
        std.mem.eql(u8, admission.sessionRef, session.sessionRef);
}

fn retainCodingSession(model: *Model, session: CodingSessionWire) bool {
    const grant_ref = session.grantRef orelse return false;
    const work_context_ref = session.workContextRef orelse return false;
    return copyRef(&model.grant_ref_storage, &model.grant_ref_len, grant_ref) and
        copyRef(&model.project_ref_storage, &model.project_ref_len, session.projectRef) and
        copyRef(&model.repository_ref_storage, &model.repository_ref_len, session.repositoryRef) and
        copyRef(&model.worktree_ref_storage, &model.worktree_ref_len, session.worktreeRef) and
        copyRef(&model.work_context_ref_storage, &model.work_context_ref_len, work_context_ref) and
        copyRef(&model.session_ref_storage, &model.session_ref_len, session.sessionRef);
}

fn applySidecarRpcResult(model: *Model, response: native_sdk.EffectResponse) void {
    if (response.key != sidecar_rpc_effect_key or response.outcome != .ok or response.status != 200 or
        response.truncated or response.dropped_before != 0 or response.body.len == 0 or
        response.body.len > sidecar_frame_limit)
    {
        model.repository_phase = .refused;
        return;
    }
    var parsed = std.json.parseFromSlice(CodingRpcResponseWire, std.heap.page_allocator, response.body, .{}) catch {
        model.repository_phase = .refused;
        return;
    };
    defer parsed.deinit();
    const wire = parsed.value;
    var expected_request_id_buffer: [96]u8 = undefined;
    const expected_request_id = std.fmt.bufPrint(
        &expected_request_id_buffer,
        "native.coding.{d}",
        .{model.repository_request_sequence},
    ) catch {
        model.repository_phase = .refused;
        return;
    };
    if (!std.mem.eql(u8, wire.protocol, sidecar_rpc_protocol) or
        wire.generation != model.sidecar_generation or
        !std.mem.eql(u8, wire.nonce, model.sidecar_nonce) or
        !std.mem.eql(u8, wire.requestId, expected_request_id) or
        wire.result.projectionDigest.len != 71 or
        !std.mem.startsWith(u8, wire.result.projectionDigest, "sha256:") or
        !std.mem.eql(u8, wire.result.projection.authority, "device_local") or
        !std.mem.eql(u8, wire.result.projection.authorityLabel, "This Mac") or
        wire.result.projection.pageOffset != 0 or
        wire.result.projection.totalSessions < wire.result.projection.sessions.len)
    {
        model.repository_phase = .refused;
        return;
    }
    for (wire.result.projectionDigest[7..]) |character| {
        if (!std.ascii.isDigit(character) and (character < 'a' or character > 'f')) {
            model.repository_phase = .refused;
            return;
        }
    }
    if (std.mem.eql(u8, wire.result.kind, "coding.refused")) {
        model.repository_phase = .refused;
        return;
    }
    model.catalog_total_sessions = wire.result.projection.totalSessions;
    const selected = wire.result.projection.selectedSessionRef orelse {
        model.repository_phase = .empty;
        return;
    };
    if (wire.result.projection.totalSessions == 0 or wire.result.projection.sessions.len == 0) {
        model.repository_phase = .refused;
        return;
    }
    const session = for (wire.result.projection.sessions) |candidate| {
        if (std.mem.eql(u8, candidate.sessionRef, selected)) break candidate;
    } else {
        model.repository_phase = .refused;
        return;
    };
    if (std.mem.eql(u8, wire.result.kind, "coding.admitted")) {
        const admission = wire.result.admission orelse {
            model.repository_phase = .refused;
            return;
        };
        if (!admissionMatchesSession(admission, session)) {
            model.repository_phase = .refused;
            return;
        }
    } else if (!std.mem.eql(u8, wire.result.kind, "coding.snapshot") or wire.result.admission != null) {
        model.repository_phase = .refused;
        return;
    }
    model.repository_phase = if (retainCodingSession(model, session)) .ready else .refused;
}

fn intentDetail(model: *const Model, output: []u8) ![]const u8 {
    const sequence = model.outbound_sequence;
    return switch (model.outbound_intent) {
        .none => error.NoIntent,
        .new_chat => std.fmt.bufPrint(output, "{{\"protocol\":1,\"sequence\":{d},\"intent\":{{\"_tag\":\"NewChatRequested\",\"commandId\":\"chat.new\"}}}}", .{sequence}),
        .new_chat_menu => std.fmt.bufPrint(output, "{{\"protocol\":1,\"sequence\":{d},\"intent\":{{\"_tag\":\"DeferredCommand\",\"command\":{{\"schema\":\"openagents.desktop.deferred_command.v1\",\"requestRef\":\"command.native-sdk.menu.{d}\",\"commandId\":\"chat.new\",\"arguments\":{{\"kind\":\"none\"}},\"source\":\"native_menu\",\"delivery\":\"dispatch\"}}}}}}", .{ sequence, sequence }),
        .reload_effect => std.fmt.bufPrint(output, "{{\"protocol\":1,\"sequence\":{d},\"intent\":{{\"_tag\":\"RendererReloadRequested\",\"commandId\":\"openagents.spike.reload-effect\"}}}}", .{sequence}),
        .workspace_chat => std.fmt.bufPrint(output, "{{\"protocol\":1,\"sequence\":{d},\"intent\":{{\"_tag\":\"WorkspaceSelected\",\"workspace\":\"chat\",\"commandId\":\"chat.open\"}}}}", .{sequence}),
        .workspace_home => std.fmt.bufPrint(output, "{{\"protocol\":1,\"sequence\":{d},\"intent\":{{\"_tag\":\"WorkspaceSelected\",\"workspace\":\"home\",\"commandId\":\"workspace.home\"}}}}", .{sequence}),
        .workspace_settings => std.fmt.bufPrint(output, "{{\"protocol\":1,\"sequence\":{d},\"intent\":{{\"_tag\":\"WorkspaceSelected\",\"workspace\":\"settings\",\"commandId\":\"settings.open\"}}}}", .{sequence}),
        .session_parity => std.fmt.bufPrint(output, "{{\"protocol\":1,\"sequence\":{d},\"intent\":{{\"_tag\":\"SessionSelected\",\"sessionRef\":\"session.parity\",\"commandId\":null}}}}", .{sequence}),
        .session_renderer => std.fmt.bufPrint(output, "{{\"protocol\":1,\"sequence\":{d},\"intent\":{{\"_tag\":\"SessionSelected\",\"sessionRef\":\"session.renderer\",\"commandId\":null}}}}", .{sequence}),
        .session_audit => std.fmt.bufPrint(output, "{{\"protocol\":1,\"sequence\":{d},\"intent\":{{\"_tag\":\"SessionSelected\",\"sessionRef\":\"session.audit\",\"commandId\":null}}}}", .{sequence}),
    };
}

fn retainRepositoryRoot(model: *Model, root: []const u8) bool {
    if (root.len == 0 or root.len > model.repository_root_storage.len or root[0] != '/') return false;
    if (std.mem.indexOfScalar(u8, root, 0) != null or std.mem.indexOfScalar(u8, root, '\n') != null) return false;
    @memcpy(model.repository_root_storage[0..root.len], root);
    model.repository_root_len = root.len;
    model.repository_dialog_requested = false;
    return true;
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
            .command => |platform_command| if (std.mem.eql(u8, platform_command.name, reload_command)) {
                try self.ui.dispatch(runtime, if (platform_command.window_id == 0) 1 else platform_command.window_id, .reload_effect_surface);
                return;
            },
            .canvas_widget_file_drop => |drop| {
                if (std.mem.eql(u8, drop.view_label, canvas_label) and drop.target != null and drop.drop.paths.len == 1) {
                    if (!retainRepositoryRoot(&self.ui.model, drop.drop.paths[0])) return error.InvalidRepositorySelection;
                    try self.ui.dispatch(runtime, drop.window_id, .request_repository_admit);
                }
            },
            else => {},
        }
        try self.inner.event(runtime, value);
        if (self.ui.model.repository_dialog_requested) {
            self.ui.model.repository_dialog_requested = false;
            var dialog_buffer: [4096]u8 = undefined;
            const selection = try runtime.showOpenDialog(.{
                .title = "Grant a Git repository to OpenAgents",
                .allow_directories = true,
                .allow_multiple = false,
            }, &dialog_buffer);
            if (selection.count == 1 and retainRepositoryRoot(&self.ui.model, selection.paths)) {
                try self.ui.dispatch(runtime, 1, .request_repository_admit);
            } else {
                self.ui.model.repository_phase = .empty;
            }
        }
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
        if (acknowledged > self.ui.model.outbound_sequence) {
            // A restarted Native process has no pending intent but the
            // persisted child may carry the prior process's acknowledged
            // sequence fence. Adopt it exactly once before any local command
            // so subsequent commands remain globally monotonic.
            if (self.ui.model.outbound_sequence == 0 and self.ui.model.outbound_intent == .none) {
                self.ui.model.outbound_sequence = acknowledged;
            } else return error.InvalidProjection;
        }
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
    var frontend_url_buffer: [2048]u8 = undefined;
    var sidecar_data_root_buffer: [4096]u8 = undefined;
    var sidecar_state_root_buffer: [4096]u8 = undefined;
    var sidecar_token_bytes: [32]u8 = undefined;
    try std.Io.randomSecure(init.io, &sidecar_token_bytes);
    const sidecar_token_hex = std.fmt.bytesToHex(sidecar_token_bytes, .lower);
    const frontend_base_url = init.environ_map.get("NATIVE_SDK_FRONTEND_URL") orelse effect_native_base_url;
    const run_namespace = init.environ_map.get("NATIVE_SDK_ASSURANCE_RUN_NONCE");
    model.sidecar_node_path = init.environ_map.get("OPENAGENTS_NATIVE_NODE_PATH") orelse "";
    model.sidecar_entry_path = init.environ_map.get("OPENAGENTS_NATIVE_SIDECAR_PATH") orelse "";
    model.sidecar_state_root = init.environ_map.get("OPENAGENTS_NATIVE_STATE_ROOT") orelse blk: {
        const data_root = try native_sdk.app_dirs.resolveOne(
            .{ .name = "OpenAgents" },
            native_sdk.app_dirs.currentPlatform(),
            native_sdk.debug.envFromMap(init.environ_map),
            .data,
            &sidecar_data_root_buffer,
        );
        break :blk try native_sdk.app_dirs.join(
            native_sdk.app_dirs.currentPlatform(),
            &sidecar_state_root_buffer,
            &.{ data_root, "native-sidecar" },
        );
    };
    model.sidecar_token = &sidecar_token_hex;
    model.sidecar_nonce = run_namespace orelse "native.local";
    if (!validRunNamespace(model.sidecar_nonce)) return error.InvalidSidecarNonce;
    if (init.environ_map.get("NATIVE_SDK_SIDECAR_GENERATION")) |raw_generation| {
        model.sidecar_generation = try std.fmt.parseUnsigned(u64, raw_generation, 10);
        if (model.sidecar_generation == 0) return error.InvalidSidecarGeneration;
    }
    model.frontend_url = if (run_namespace) |namespace|
        if (validRunNamespace(namespace))
            try std.fmt.bufPrint(&frontend_url_buffer, "{s}#surface=effect-native&assurance-run={s}", .{ frontend_base_url, namespace })
        else
            return error.InvalidRunNamespace
    else
        try std.fmt.bufPrint(&frontend_url_buffer, "{s}#surface=effect-native", .{frontend_base_url});
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
        .menus = &app_menus,
    }, init);
}

test {
    _ = @import("tests.zig");
}
