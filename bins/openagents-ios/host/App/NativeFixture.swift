// A developer screen that renders Rust Native's conversation fixture, so the
// conversation elements can be checked without a paired computer. Launch a
// simulator or debug build with `--rust-native-fixture` to show it. Add
// `--rust-native-fixture-rows N` to append N synthetic rows,
// `--rust-native-fixture-demo` to expand a tool and stream replies,
// `--rust-native-transcript-bench` to fling through the transcript and print
// frame times, `--rust-native-transcript-log` to print layout timings, and
// `--rust-native-transcript-pull` to publish the rows to a Rust transcript
// source and render the transcript from it, as the app's chats do.
import SwiftUI

enum NativeFixture {
    static var requested: Bool {
        #if DEBUG || targetEnvironment(simulator) || RUST_NATIVE_BENCH
        return ProcessInfo.processInfo.arguments.contains("--rust-native-fixture")
        #else
        return false
        #endif
    }
}

extension View {
    /// Replaces this screen with the fixture when the launch asks for it.
    @ViewBuilder func nativeFixture() -> some View {
        if NativeFixture.requested { NativeFixtureScreen() } else { self }
    }
}

private struct NativeFixtureScreen: View {
    @State private var view: NativeView?
    /// What shows: `view`, or with `--rust-native-transcript-pull`, `view`
    /// once its rows are published to a Rust transcript source.
    @State private var shown: NativeView?
    @State private var failure: String?
    private static let publisher = DispatchQueue(label: "com.openagents.fixture-publish")
    /// The newest revision asked for; older queued publications skip.
    nonisolated(unsafe) private static var newest: UInt64 = 0
    private static let newestLock = NSLock()

    var body: some View {
        ZStack {
            Color.black.ignoresSafeArea()
            if let view, let shown {
                NativeRenderer(node: shown.root, revision: shown.revision, followTarget: nil,
                               followChanged: nil,
                               submit: { token, text in
                                   print("fixture submit \(token): \(text)")
                                   self.view = view.appending(text)
                               },
                               activate: { node in
                                   print("fixture activate \(node)")
                                   if node == "transcript" { self.view = view.prependingEarlier() }
                               })
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
            } else if let failure {
                Text(failure).font(.caption).foregroundStyle(.white).padding()
            }
        }
        .preferredColorScheme(.dark)
        .onAppear(perform: load)
        .onChange(of: view?.revision) { publish() }
        .task { await demo() }
    }

    /// Shows `view`. Publishing encodes every row, as the application's Rust
    /// would on its own queue, so it runs off the main thread; a revision
    /// that a newer one overtook is not shown.
    private func publish() {
        guard let view else { return }
        guard ProcessInfo.processInfo.arguments.contains("--rust-native-transcript-pull") else {
            shown = view
            return
        }
        Self.newestLock.withLock { Self.newest = view.revision }
        Self.publisher.async {
            guard Self.newestLock.withLock({ Self.newest == view.revision }) else { return }
            let pulled = view.pulled()
            DispatchQueue.main.async {
                if self.view?.revision == pulled.revision { self.shown = pulled }
            }
        }
    }

    /// With `--rust-native-fixture-demo`, expands the tool row and streams
    /// replies so following can be checked without touching the screen.
    private func demo() async {
        guard ProcessInfo.processInfo.arguments.contains("--rust-native-fixture-demo") else { return }
        try? await Task.sleep(for: .seconds(1))
        NativeExpansion.shared.toggle("t1")
        try? await Task.sleep(for: .seconds(3))
        for index in 1...5 {
            try? await Task.sleep(for: .milliseconds(400))
            view = view?.appending("Demo message \(index)")
        }
        // Stream a reply a few words at a time; the transcript stays pinned.
        let words = ("Streaming a reply one token at a time keeps the newest row in view. "
                     + "Rust re-measures only this row for each token, and the rows above keep their places. ")
            .split(separator: " ")
        var streamed = ""
        for index in 0..<(words.count * 3) {
            try? await Task.sleep(for: .milliseconds(60))
            streamed += (streamed.isEmpty ? "" : " ") + words[index % words.count]
            view = view?.streaming(streamed)
        }
    }

    /// The count after `--rust-native-fixture-rows`, at most 20,000.
    private static var syntheticRows: Int {
        let arguments = ProcessInfo.processInfo.arguments
        guard let index = arguments.firstIndex(of: "--rust-native-fixture-rows"), index + 1 < arguments.count,
              let count = Int(arguments[index + 1]) else { return 0 }
        return min(max(count, 0), 20_000)
    }

    private func load() {
        do {
            guard let url = Bundle.main.url(forResource: "conversation", withExtension: "json") else {
                failure = "The fixture isn't in the app bundle."
                return
            }
            let fixture = try JSONDecoder().decode(NativeView.self, from: Data(contentsOf: url))
            view = fixture.addingSynthetic(Self.syntheticRows)
        } catch {
            failure = "Couldn't decode the fixture: \(error)"
        }
    }
}

/// Local stand-ins for the application, so a reader can exercise sending,
/// following, and loading older rows against the fixture.
private extension NativeView {
    private static let plain = NativeStyle(foreground: nil, background: nil, padding_top: nil,
                                           padding_end: nil, padding_bottom: nil, padding_start: nil,
                                           gap: nil, weight: nil, align: nil)

    private static func message(_ key: String, role: String, _ text: String) -> NativeNode {
        let span = try? JSONDecoder().decode(NativeMarkdownSpan.self,
                                             from: JSONEncoder().encode(["text": text]))
        return NativeNode(key: key, style: plain, element: .message(role, nil, [
            NativeNode(key: "\(key)-text", style: plain, element: .markdown(span.map { [.paragraph([$0])] } ?? [])),
        ]))
    }

    /// With `--rust-native-transcript-pull`, publishes the transcript's rows
    /// to a Rust transcript source and returns the view with the rows
    /// replaced by the source's name.
    func pulled() -> NativeView {
        guard ProcessInfo.processInfo.arguments.contains("--rust-native-transcript-pull"),
              case let .stack(axis, children) = root.element else { return self }
        let mapped = children.map { child -> NativeNode in
            guard case let .transcript(label, _, earlier, nil) = child.element,
                  let data = try? JSONEncoder().encode(child) else { return child }
            let name = Array("fixture:\(child.key)".utf8)
            let published = data.withUnsafeBytes { bytes in
                rust_native_source_publish(name, name.count, bytes.bindMemory(to: UInt8.self).baseAddress, bytes.count)
            }
            guard published == 1 else { return child }
            return NativeNode(key: child.key, style: child.style,
                              element: .transcript(label, [], earlier, String(decoding: name, as: UTF8.self)))
        }
        return NativeView(schema: schema, instance: instance, revision: revision,
                          root: NativeNode(key: root.key, style: root.style, element: .stack(axis, mapped)))
    }

    func mapTranscript(_ change: ([NativeNode]) -> [NativeNode]) -> NativeView {
        guard case let .stack(axis, children) = root.element else { return self }
        let mapped = children.map { child -> NativeNode in
            guard case let .transcript(label, rows, earlier, source) = child.element else { return child }
            return NativeNode(key: child.key, style: child.style,
                              element: .transcript(label, change(rows), earlier, source))
        }
        return NativeView(schema: schema, instance: instance, revision: revision + 1,
                          root: NativeNode(key: root.key, style: root.style, element: .stack(axis, mapped)))
    }

    func appending(_ text: String) -> NativeView {
        let id = revision + 1
        return mapTranscript { rows in
            rows.filter { $0.key != "working" } + [
                Self.message("sent-\(id)", role: "user", text),
                Self.message("reply-\(id)", role: "assistant",
                             "This is a local reply from the fixture screen. " + String(repeating: "More text to fill the row. ", count: 6)),
            ] + rows.filter { $0.key == "working" }
        }
    }

    /// Replaces the streaming reply's text, adding the reply first.
    func streaming(_ text: String) -> NativeView {
        mapTranscript { rows in
            rows.filter { $0.key != "working" && $0.key != "stream" }
                + [Self.message("stream", role: "assistant", text)]
                + rows.filter { $0.key == "working" }
        }
    }

    /// Synthetic rows before the fixture's own, to check scrolling and layout
    /// cost over a long transcript.
    func addingSynthetic(_ count: Int) -> NativeView {
        guard count > 0 else { return self }
        var rows: [[String: Any]] = []
        func span(_ text: String, _ flags: [String: Any] = [:]) -> [String: Any] {
            flags.merging(["text": text]) { current, _ in current }
        }
        func message(_ key: String, _ role: String, _ blocks: [[String: Any]]) -> [String: Any] {
            ["key": key, "style": [:] as [String: Any], "element": ["kind": "message", "props": [
                "role": role, "note": NSNull(), "children": [
                    ["key": "\(key)-md", "style": [:] as [String: Any],
                     "element": ["kind": "markdown", "props": ["blocks": blocks]]],
                ],
            ]]]
        }
        for index in 0..<count {
            let key = "s\(index)"
            switch index % 5 {
            case 0:
                rows.append(message(key, "user", [["kind": "paragraph", "spans": [
                    span("Question \(index): why does the "), span("scheduler", ["code": true]),
                    span(" stall under load?"),
                ]]]))
            case 1:
                rows.append(["key": key, "style": [:] as [String: Any], "element": ["kind": "tool", "props": [
                    "name": "Bash", "detail": "rg -n stall crates/scheduler \(index)", "state": "done",
                    "children": [["key": "\(key)-out", "style": [:] as [String: Any], "element": [
                        "kind": "text", "props": ["value": "crates/scheduler/src/lib.rs:\(index): stall", "role": "code"],
                    ]]],
                ]]])
            case 2:
                rows.append(message(key, "assistant", [
                    ["kind": "paragraph", "spans": [
                        span("The queue holds a lock while it "), span("waits", ["italic": true]),
                        span(" for the next job, so every worker blocks behind it. Row \(index) shows the same pattern: "),
                        span("release the lock before waiting", ["bold": true]), span("."),
                    ]],
                    ["kind": "list", "ordered": true, "start": 1, "items": [
                        ["blocks": [["kind": "paragraph", "spans": [span("Take the job under the lock.")]]]],
                        ["blocks": [["kind": "paragraph", "spans": [span("Wait on the condition without it.")]]]],
                    ]],
                ]))
            case 3:
                rows.append(message(key, "assistant", [
                    // The long line scrolls sideways.
                    ["kind": "code", "language": "rust", "text": "let job = queue.lock().expect(\"the scheduler queue lock is poisoned\").pop_front().unwrap_or_default();\ndrop(guard);\nready.wait();\n"],
                ]))
            case 4 where index % 10 == 9:
                // A table wider than the screen scrolls sideways.
                let columns = ["Worker", "Queue", "Waits", "Held for", "Jobs taken", "Stalls", "Notes"]
                rows.append(message(key, "assistant", [["kind": "table",
                    "align": columns.map { _ in "none" },
                    "header": columns.map { [span($0)] },
                    "rows": (1...3).map { row in columns.map { [span("\($0.lowercased()) \(row)")] } }]]))
            default:
                rows.append(message(key, "assistant", [["kind": "paragraph", "spans": [
                    span("Short reply \(index).")]]]))
            }
        }
        guard let data = try? JSONSerialization.data(withJSONObject: rows),
              let nodes = try? JSONDecoder().decode([NativeNode].self, from: data) else { return self }
        return mapTranscript { nodes + $0 }
    }

    func prependingEarlier() -> NativeView {
        let id = revision + 1
        return mapTranscript { rows in
            (0..<8).map { Self.message("earlier-\(id)-\($0)", role: $0 % 2 == 0 ? "user" : "assistant",
                                       "Earlier message \($0 + 1) from load \(id).") } + rows
        }
    }
}
