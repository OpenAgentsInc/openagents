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
        \\{"protocol":1,"revision":7,"workspace":"chat","selectedSessionRef":"session.renderer","messageCount":3,"pending":true,"status":"Codex is working"}
    ;
    const projection = try main.parseProjection(payload);
    var model = main.initialModel();
    main.update(&model, .{ .sync_projection = projection });

    try testing.expectEqual(@as(u64, 7), model.projection_revision);
    try testing.expectEqual(main.Session.renderer, model.session);
    try testing.expectEqual(@as(u32, 3), model.message_count);
    try testing.expect(model.pending);
    try testing.expectEqualStrings("Codex is working", model.status());

    const stale = main.Projection{ .revision = 6, .workspace = .settings, .session = .audit, .message_count = 0, .pending = false, .status = "stale" };
    main.update(&model, .{ .sync_projection = stale });
    try testing.expectEqual(main.Workspace.chat, model.workspace);
    try testing.expectEqual(@as(u64, 7), model.projection_revision);
}

test "projection protocol fails closed" {
    try testing.expectError(error.InvalidProtocol, main.parseProjection(
        \\{"protocol":2,"revision":1,"workspace":"chat","selectedSessionRef":null,"messageCount":0,"pending":false,"status":"bad"}
    ));
    try testing.expectError(error.InvalidProjection, main.parseProjection(
        \\{"protocol":1,"revision":1,"workspace":"fleet","selectedSessionRef":null,"messageCount":0,"pending":false,"status":"bad"}
    ));
    var oversized: [main.bridge_payload_limit + 1]u8 = undefined;
    @memset(&oversized, 'x');
    try testing.expectError(error.PayloadTooLarge, main.parseProjection(&oversized));
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
    try testing.expectEqual(@as(u64, 1), output[0].reload_token);
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
