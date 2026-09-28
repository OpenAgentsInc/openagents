// A host terminal, full screen. Rust owns the session, the emulator, and
// every byte sent; this view reports the grid it fits, forwards keys, and
// polls for output.
import SwiftUI

struct TerminalScreen: View {
    @ObservedObject var bridge: MobileBridge
    @State private var typing = true
    @State private var grid: (rows: Int, cols: Int) = (0, 0)

    /// Space for the terminal's title and key rows around the grid.
    private let chrome: CGFloat = 132

    var body: some View {
        GeometryReader { geometry in
            ZStack(alignment: .topLeading) {
                Color.black.ignoresSafeArea()
                TerminalKeyInput(focused: $typing,
                                 text: { bridge.terminal(["op": "terminal_text", "text": $0]) },
                                 key: { name, ctrl, alt, shift in
                                     bridge.terminal(["op": "terminal_key", "key": name,
                                                      "ctrl": ctrl, "alt": alt, "shift": shift])
                                 },
                                 paste: { bridge.terminal(["op": "terminal_paste", "text": $0]) })
                    .frame(width: 1, height: 1).opacity(0.01)
                if let view = bridge.terminalView {
                    NativeRenderer(node: view.root, revision: view.revision, followTarget: nil,
                                   followChanged: nil, activate: { node in
                                       bridge.activate("terminal", view: view, node: node)
                                   })
                        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                } else {
                    ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
                }
            }
            .contentShape(Rectangle())
            .onTapGesture { typing = true }
            .onAppear { resize(geometry.size) }
            .onChange(of: geometry.size) { _, size in resize(size) }
        }
        .task {
            while !Task.isCancelled {
                bridge.pollTerminal()
                try? await Task.sleep(for: .milliseconds(120))
            }
        }
    }

    private func resize(_ size: CGSize) {
        let cell = TerminalMetrics.cell
        let rows = max(4, min(200, Int((size.height - chrome) / cell.height)))
        let cols = max(20, min(300, Int((size.width - 16) / cell.width)))
        guard rows != grid.rows || cols != grid.cols else { return }
        grid = (rows, cols)
        bridge.terminal(["op": "terminal_resize", "rows": rows, "cols": cols])
    }
}
