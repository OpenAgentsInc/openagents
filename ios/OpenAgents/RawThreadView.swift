import SwiftUI

struct RawThreadView: View {
    let url: URL?

    var body: some View {
        if let u = url, let text = try? String(contentsOf: u) {
            ScrollView {
                Text(text)
                    .textSelection(.enabled)
                    .font(.system(.body, design: .monospaced))
                    .padding()
            }
        } else {
            Text("Select a thread")
                .font(.headline)
                .foregroundStyle(.secondary)
        }
    }
}

#Preview { RawThreadView(url: nil) }

