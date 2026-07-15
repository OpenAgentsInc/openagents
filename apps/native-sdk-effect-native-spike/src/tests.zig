const std = @import("std");
const native_sdk = @import("native_sdk");
const main = @import("main.zig");

const canvas = native_sdk.canvas;
const testing = std.testing;

fn buildTree(arena: std.mem.Allocator, model: *const main.Model) !main.AppUi.Tree {
    var ui = main.AppUi.init(arena);
    return ui.finalize(main.view(&ui, model));
}

fn findByText(widget: canvas.Widget, kind: canvas.WidgetKind, text: []const u8) ?canvas.Widget {
    if (widget.kind == kind and std.mem.eql(u8, widget.text, text)) return widget;
    for (widget.children) |child| if (findByText(child, kind, text)) |found| return found;
    return null;
}

fn expectByText(widget: canvas.Widget, kind: canvas.WidgetKind, text: []const u8) !canvas.Widget {
    return findByText(widget, kind, text) orelse error.WidgetNotFound;
}

test "native selection waits for an Effect projection" {
    var arena_state = std.heap.ArenaAllocator.init(testing.allocator);
    defer arena_state.deinit();

    var model = main.initialModel();
    const tree = try buildTree(arena_state.allocator(), &model);
    const renderer = try expectByText(tree.root, .list_item, "Renderer boundary");
    main.update(&model, tree.msgForPointer(renderer.id, .up).?);

    try testing.expectEqual(main.Session.parity, model.session);
    try testing.expectEqual(main.OutboundIntent.session_renderer, model.outbound_intent);
    try testing.expectEqual(@as(u64, 1), model.outbound_sequence);
    try testing.expect(model.awaiting_projection);
}

test "bounded projection updates the native mirror and rejects stale state" {
    const payload =
        \\{"protocol":1,"revision":7,"workspace":"chat","selectedSessionRef":"session.renderer","messageCount":3,"pending":true,"status":"Codex is working","lastAppliedCommand":null}
    ;
    const projection = try main.parseProjection(payload);
    var model = main.initialModel();
    main.update(&model, .{ .sync_projection = projection });

    try testing.expectEqual(@as(u64, 7), model.projection_revision);
    try testing.expectEqual(main.Session.renderer, model.session);
    try testing.expectEqual(@as(u32, 3), model.message_count);
    try testing.expect(model.pending);
    try testing.expectEqualStrings("Codex is working", model.status());

    const stale = main.Projection{ .revision = 6, .workspace = .settings, .session = .audit, .message_count = 0, .pending = false, .status = "stale", .last_applied_command_sequence = 0 };
    main.update(&model, .{ .sync_projection = stale });
    try testing.expectEqual(main.Workspace.chat, model.workspace);
    try testing.expectEqual(@as(u64, 7), model.projection_revision);
}

test "projection protocol fails closed" {
    try testing.expectError(error.InvalidProtocol, main.parseProjection(
        \\{"protocol":2,"revision":1,"workspace":"chat","selectedSessionRef":null,"messageCount":0,"pending":false,"status":"bad","lastAppliedCommand":null}
    ));
    try testing.expectError(error.InvalidProjection, main.parseProjection(
        \\{"protocol":1,"revision":1,"workspace":"fleet","selectedSessionRef":null,"messageCount":0,"pending":false,"status":"bad","lastAppliedCommand":null}
    ));
    var oversized: [main.bridge_payload_limit + 1]u8 = undefined;
    @memset(&oversized, 'x');
    try testing.expectError(error.PayloadTooLarge, main.parseProjection(&oversized));
}

test "native New Chat menu and shortcut share the canonical command" {
    try testing.expectEqual(main.Msg.request_new_chat_menu, main.command("chat.new").?);
    try testing.expect(main.command("shell.exec") == null);
    try testing.expectEqual(@as(usize, 1), main.app_menus.len);
    try testing.expectEqual(@as(usize, 1), main.file_menu_items.len);
    try testing.expectEqualStrings("New Chat", main.file_menu_items[0].label);
    try testing.expectEqualStrings("chat.new", main.file_menu_items[0].command);
    try testing.expectEqualStrings("n", main.file_menu_items[0].key);
    try testing.expect(main.file_menu_items[0].modifiers.command);
}

test "headed storage namespaces are bounded before entering the child URL" {
    try testing.expect(main.validRunNamespace("18d940df-9c48-480a-b66b-c086c67442a6"));
    try testing.expect(main.validRunNamespace("proof.42_native"));
    try testing.expect(!main.validRunNamespace("../../shared"));
    try testing.expect(!main.validRunNamespace(""));
}

test "applied production command metadata is exact" {
    const projection = try main.parseProjection(
        \\{"protocol":1,"revision":8,"workspace":"chat","selectedSessionRef":null,"messageCount":0,"pending":false,"status":"Production Desktop shell synchronized","lastAppliedCommand":{"sequence":4,"commandId":"chat.new","intentName":"DesktopNewChat","source":"native_menu"}}
    );
    try testing.expectEqual(@as(u64, 4), projection.last_applied_command_sequence);
    try testing.expectError(error.InvalidProjection, main.parseProjection(
        \\{"protocol":1,"revision":8,"workspace":"chat","selectedSessionRef":null,"messageCount":0,"pending":false,"status":"bad","lastAppliedCommand":{"sequence":4,"commandId":"chat.create","intentName":"DesktopNewChat","source":"native_menu"}}
    ));
}

test "exact Node sidecar bootstrap uses the Native SDK effects channel" {
    var model = main.initialModel();
    model.sidecar_node_path = "/opt/openagents/node";
    model.sidecar_entry_path = "/opt/openagents/native-sidecar.mjs";
    model.sidecar_nonce = "proof.native_1";
    model.sidecar_generation = 3;
    model.sidecar_state_root = "/tmp/openagents-native-state";
    model.sidecar_token = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    var fx = main.Effects.init(testing.allocator);
    defer fx.deinit();
    fx.executor = .fake;

    main.initEffects(&model, &fx);

    try testing.expectEqual(main.SidecarPhase.starting, model.sidecar_phase);
    try testing.expectEqual(@as(usize, 1), fx.pendingSpawnCount());
    const request = fx.pendingSpawnAt(0).?;
    try testing.expectEqual(main.sidecar_effect_key, request.key);
    try testing.expectEqual(native_sdk.EffectOutputMode.lines, request.output);
    try testing.expectEqualStrings("/opt/openagents/node", request.argv[0]);
    try testing.expectEqualStrings("/opt/openagents/native-sidecar.mjs", request.argv[1]);
    try testing.expectEqualStrings(
        "{\"protocol\":\"openagents.desktop.native-sidecar.v2\",\"generation\":3,\"nonce\":\"proof.native_1\",\"stateRootBase64\":\"L3RtcC9vcGVuYWdlbnRzLW5hdGl2ZS1zdGF0ZQ==\",\"transportToken\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}",
        request.stdin,
    );
}

test "sidecar bootstrap receipt is generation-fenced and fails closed" {
    const receipt =
        \\{"protocol":"openagents.desktop.native-sidecar.v2","generation":2,"nonce":"proof.native_2","pid":4242,"nodeVersion":"24.13.1","gatewayProtocolVersion":11,"requestId":"native-sidecar.bootstrap","response":{"kind":"query_result","requestId":"native-sidecar.bootstrap","result":{"kind":"runtime.bootstrap","protocolVersion":11,"lifecycle":"ready","sessionPhase":"unavailable","identityTier":"local_unavailable","capabilities":[]}},"transport":{"kind":"loopback_http","host":"127.0.0.1","port":43123}}
    ;
    const parsed = try main.parseSidecarBootstrapReceipt(receipt, 2, "proof.native_2");
    try testing.expectEqual(@as(u64, 2), parsed.generation);
    try testing.expectEqual(@as(u32, 4242), parsed.pid);
    try testing.expectEqual(@as(u8, 11), parsed.gateway_protocol);
    try testing.expectEqual(@as(u16, 43123), parsed.port);
    try testing.expectError(error.InvalidSidecarReceipt, main.parseSidecarBootstrapReceipt(receipt, 1, "proof.native_2"));
    const excess = try std.fmt.allocPrint(testing.allocator, "{s},\"ambientPath\":\"/private/repository\"}}", .{receipt[0 .. receipt.len - 1]});
    defer testing.allocator.free(excess);
    try testing.expectError(error.InvalidSidecarReceipt, main.parseSidecarBootstrapReceipt(excess, 2, "proof.native_2"));
    const duplicate = try std.fmt.allocPrint(testing.allocator, "{s},\"generation\":2}}", .{receipt[0 .. receipt.len - 1]});
    defer testing.allocator.free(duplicate);
    try testing.expectError(error.InvalidSidecarReceipt, main.parseSidecarBootstrapReceipt(duplicate, 2, "proof.native_2"));

    var model = main.initialModel();
    model.sidecar_nonce = "proof.native_2";
    model.sidecar_generation = 2;
    model.sidecar_phase = .starting;
    main.update(&model, .{ .sidecar_ready_line = .{
        .key = main.sidecar_effect_key,
        .line = receipt,
    } });
    try testing.expectEqual(main.SidecarPhase.ready, model.sidecar_phase);
    try testing.expectEqual(@as(u32, 4242), model.sidecar_pid);

    main.update(&model, .{ .sidecar_exited = .{
        .key = main.sidecar_effect_key,
        .code = 1,
        .reason = .exited,
    } });
    try testing.expectEqual(main.SidecarPhase.unavailable, model.sidecar_phase);
}

test "web pane stays anchored to the full-height Effect Native surface" {
    var model = main.initialModel();
    var output: [1]main.SpikeApp.WebViewPane = undefined;
    try testing.expectEqual(@as(usize, 1), main.panes(&model, &output));
    try testing.expectEqualStrings(main.webview_label, output[0].label);
    try testing.expectEqualStrings(main.pane_anchor, output[0].anchor.?);
    try testing.expectEqualStrings(main.effect_native_url, output[0].url);
    main.update(&model, .reload_effect_surface);
    _ = main.panes(&model, &output);
    try testing.expectEqual(@as(u64, 0), output[0].reload_token);
    try testing.expectEqualStrings(main.effect_native_url, output[0].url);
    try testing.expectEqual(main.OutboundIntent.reload_effect, model.outbound_intent);
    try testing.expectEqual(@as(u64, 1), model.outbound_sequence);
    try testing.expect(model.awaiting_projection);
}

test "the Native SDK catalog retains the proposed Effect Native lowerings" {
    try testing.expect(canvas.builtinComponentCount() >= 32);
    try testing.expectEqualStrings("Button", canvas.builtinComponentName(.button));
    try testing.expectEqualStrings("Card", canvas.builtinComponentName(.card));
    try testing.expectEqualStrings("Resizable", canvas.builtinComponentName(.resizable));
    try testing.expectEqualStrings("Table", canvas.builtinComponentName(.table));
    try testing.expect(canvas.builtinComponentDescriptor(.select).composite);
    try testing.expect(!canvas.builtinComponentDescriptor(.badge).composite);
}

test "product-shaped shell lays out through the retained canvas engine" {
    var arena_state = std.heap.ArenaAllocator.init(testing.allocator);
    defer arena_state.deinit();
    var model = main.initialModel();
    const tree = try buildTree(arena_state.allocator(), &model);
    var nodes: [256]canvas.WidgetLayoutNode = undefined;
    const layout = try canvas.layoutWidgetTree(tree.root, native_sdk.geometry.RectF.init(0, 0, 1200, 800), &nodes);
    try testing.expect(layout.nodes.len > 12);
    _ = try expectByText(tree.root, .button, "New chat");
    _ = try expectByText(tree.root, .list_item, "Chat");
    _ = try expectByText(tree.root, .list_item, "Settings");
}
