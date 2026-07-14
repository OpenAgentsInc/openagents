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
    for (widget.children) |child| {
        if (findByText(child, kind, text)) |found| return found;
    }
    return null;
}

fn expectByText(widget: canvas.Widget, kind: canvas.WidgetKind, text: []const u8) !canvas.Widget {
    return findByText(widget, kind, text) orelse {
        std.debug.print("no {t} with text \"{s}\" in the Native SDK view\n", .{ kind, text });
        return error.WidgetNotFound;
    };
}

test "native opinionated controls dispatch through the typed update loop" {
    var arena_state = std.heap.ArenaAllocator.init(testing.allocator);
    defer arena_state.deinit();

    var model = main.initialModel();
    var tree = try buildTree(arena_state.allocator(), &model);

    const increment = try expectByText(tree.root, .button, "Increment native");
    main.update(&model, tree.msgForPointer(increment.id, .up).?);
    try testing.expectEqual(@as(i64, 1), model.native_count);

    tree = try buildTree(arena_state.allocator(), &model);
    try testing.expectEqual(increment.id, (try expectByText(tree.root, .button, "Increment native")).id);

    const toggle = try expectByText(tree.root, .switch_control, "Show adoption details");
    main.update(&model, tree.msgForPointer(toggle.id, .up).?);
    try testing.expect(!model.details_visible);
}

test "web pane stays anchored to the Effect Native surface" {
    var model = main.initialModel();
    var output: [1]main.SpikeApp.WebViewPane = undefined;

    try testing.expectEqual(@as(usize, 1), main.panes(&model, &output));
    try testing.expectEqualStrings(main.webview_label, output[0].label);
    try testing.expectEqualStrings(main.pane_anchor, output[0].anchor.?);
    try testing.expectEqualStrings(main.effect_native_url, output[0].url);

    main.update(&model, .reload_effect_surface);
    _ = main.panes(&model, &output);
    try testing.expectEqual(@as(u64, 1), output[0].reload_token);

    model.frontend_url = "http://127.0.0.1:5173/";
    _ = main.panes(&model, &output);
    try testing.expectEqualStrings("http://127.0.0.1:5173/", output[0].url);
}

test "the Native SDK component catalog exposes the adoption targets" {
    try testing.expect(canvas.builtinComponentCount() >= 32);
    try testing.expectEqualStrings("Button", canvas.builtinComponentName(.button));
    try testing.expectEqualStrings("Card", canvas.builtinComponentName(.card));
    try testing.expectEqualStrings("Resizable", canvas.builtinComponentName(.resizable));
    try testing.expectEqualStrings("Table", canvas.builtinComponentName(.table));
    try testing.expect(canvas.builtinComponentDescriptor(.select).composite);
    try testing.expect(!canvas.builtinComponentDescriptor(.badge).composite);
}

test "the hybrid shell lays out through the retained canvas engine" {
    var arena_state = std.heap.ArenaAllocator.init(testing.allocator);
    defer arena_state.deinit();

    var model = main.initialModel();
    const tree = try buildTree(arena_state.allocator(), &model);
    var nodes: [256]canvas.WidgetLayoutNode = undefined;
    const layout = try canvas.layoutWidgetTree(
        tree.root,
        native_sdk.geometry.RectF.init(0, 0, 1120, 720),
        &nodes,
    );

    try testing.expect(layout.nodes.len > 12);
    _ = try expectByText(tree.root, .badge, "NATIVE");
    _ = try expectByText(tree.root, .button, "Reload Effect pane");
}
