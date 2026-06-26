import SwiftUI

/// A single chat turn in the ChatGPT-style layout.
///
/// - Assistant turns are **full-width** with rich markdown (`MarkdownMessage`)
///   and a response action row (copy). Fenced code blocks render as scrollable
///   monospace `CodeBlockView`s with their own copy button.
/// - User turns are **compact**, right-aligned, plain-text bubbles.
///
/// This is the rich-rendering entry point that `ChatView` swaps in for the
/// foundation's plain-text bubble.
struct MessageBubble: View {
    let title: String
    let text: String
    let outgoing: Bool
    /// When true (an in-flight assistant turn), the response action row is
    /// hidden until the stream settles.
    var isStreaming: Bool = false

    var body: some View {
        if outgoing {
            userBubble
        } else {
            assistantTurn
        }
    }

    // MARK: - User (compact, right-aligned)

    private var userBubble: some View {
        HStack {
            Spacer(minLength: 40)
            VStack(alignment: .leading, spacing: 6) {
                Text(title)
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.secondary)
                Text(text)
                    .font(.body)
                    .foregroundStyle(.primary)
                    .textSelection(.enabled)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .padding(14)
            .frame(maxWidth: 520, alignment: .leading)
            .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 14))
        }
    }

    // MARK: - Assistant (full-width, markdown + actions)

    private var assistantTurn: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title)
                .font(.caption.weight(.semibold))
                .foregroundStyle(.secondary)

            MarkdownMessage(content: text)

            if !isStreaming, !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                ResponseActionRow(messageText: text)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// The response action row under an assistant turn. v1 ships a copy-message
/// action; `onRegenerate` is optional so the composer lane can wire regenerate
/// without this file owning the request path.
private struct ResponseActionRow: View {
    let messageText: String
    var onRegenerate: (() -> Void)? = nil

    @State private var copied = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        HStack(spacing: 16) {
            Button(action: copyMessage) {
                Image(systemName: copied ? "checkmark" : "doc.on.doc")
                    .font(.footnote.weight(.semibold))
                    .foregroundStyle(copied ? Color.green : Color.secondary)
            }
            .buttonStyle(.plain)
            .accessibilityLabel(copied ? "Response copied" : "Copy response")

            if let onRegenerate {
                Button(action: onRegenerate) {
                    Image(systemName: "arrow.clockwise")
                        .font(.footnote.weight(.semibold))
                        .foregroundStyle(.secondary)
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Regenerate response")
            }
            Spacer()
        }
        .padding(.top, 2)
    }

    private func copyMessage() {
        UIPasteboard.general.string = messageText
        if reduceMotion {
            copied = true
        } else {
            withAnimation(.easeInOut(duration: 0.15)) { copied = true }
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.6) {
            if reduceMotion {
                copied = false
            } else {
                withAnimation(.easeInOut(duration: 0.2)) { copied = false }
            }
        }
    }
}
