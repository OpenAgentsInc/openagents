// A developer screen that renders Rust Native's conversation fixture, so the
// conversation elements can be checked without a paired computer. Launch a
// simulator or debug build with `--rust-native-fixture` to show it.
import SwiftUI

enum NativeFixture {
    static var requested: Bool {
        #if DEBUG || targetEnvironment(simulator)
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
    @State private var failure: String?

    var body: some View {
        ZStack {
            Color.black.ignoresSafeArea()
            if let view {
                NativeRenderer(node: view.root, revision: view.revision, followTarget: nil,
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
        .task { await demo() }
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
    }

    private func load() {
        do {
            guard let url = Bundle.main.url(forResource: "conversation", withExtension: "json") else {
                failure = "The fixture isn't in the app bundle."
                return
            }
            view = try JSONDecoder().decode(NativeView.self, from: Data(contentsOf: url))
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

    func mapTranscript(_ change: ([NativeNode]) -> [NativeNode]) -> NativeView {
        guard case let .stack(axis, children) = root.element else { return self }
        let mapped = children.map { child -> NativeNode in
            guard case let .transcript(label, rows, earlier) = child.element else { return child }
            return NativeNode(key: child.key, style: child.style,
                              element: .transcript(label, change(rows), earlier))
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

    func prependingEarlier() -> NativeView {
        let id = revision + 1
        return mapTranscript { rows in
            (0..<8).map { Self.message("earlier-\(id)-\($0)", role: $0 % 2 == 0 ? "user" : "assistant",
                                       "Earlier message \($0 + 1) from load \(id).") } + rows
        }
    }
}
