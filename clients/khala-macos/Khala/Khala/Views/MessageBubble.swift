import AppKit
import SwiftUI

struct MessageBubble: View {
    let title: String
    let text: String
    let outgoing: Bool
    var isStreaming = false

    var body: some View {
        if outgoing {
            HStack {
                Spacer(minLength: 80)
                VStack(alignment: .leading, spacing: 6) {
                    Text(title)
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(.secondary)
                    Text(text)
                        .textSelection(.enabled)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .padding(14)
                .frame(maxWidth: 560, alignment: .leading)
                .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 8))
            }
        } else {
            VStack(alignment: .leading, spacing: 8) {
                Text(title)
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.secondary)
                MarkdownMessage(content: text)
                if !isStreaming, !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                    Button {
                        NSPasteboard.general.clearContents()
                        NSPasteboard.general.setString(text, forType: .string)
                    } label: {
                        Label("Copy", systemImage: "doc.on.doc")
                    }
                    .buttonStyle(.plain)
                    .font(.caption.weight(.medium))
                    .foregroundStyle(.secondary)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }
}
