import SwiftUI

/// A fenced code block rendered the way the Khala coding dogfood needs it:
/// monospaced, horizontally scrollable (long lines never wrap or clip), with an
/// optional language label and a one-tap copy button. `AttributedString`
/// markdown does not handle code fences well, so this is a dedicated view.
struct CodeBlockView: View {
    let language: String?
    let code: String

    @State private var copied = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header
            Divider().overlay(Color.white.opacity(0.08))
            ScrollView(.horizontal, showsIndicators: true) {
                Text(code)
                    .font(.system(.footnote, design: .monospaced))
                    .foregroundStyle(.primary)
                    .textSelection(.enabled)
                    .padding(12)
                    .frame(minWidth: 0, alignment: .leading)
                    .fixedSize(horizontal: true, vertical: true)
            }
        }
        .background(codeBackground, in: RoundedRectangle(cornerRadius: 12))
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .strokeBorder(Color.white.opacity(0.10), lineWidth: 1)
        )
    }

    private var header: some View {
        HStack(spacing: 8) {
            Text(displayLanguage)
                .font(.caption2.weight(.semibold))
                .textCase(.uppercase)
                .foregroundStyle(.secondary)
            Spacer()
            Button(action: copy) {
                Label(copied ? "Copied" : "Copy", systemImage: copied ? "checkmark" : "doc.on.doc")
                    .font(.caption2.weight(.semibold))
                    .labelStyle(.titleAndIcon)
            }
            .buttonStyle(.plain)
            .foregroundStyle(copied ? Color.green : Color.secondary)
            .accessibilityLabel(copied ? "Code copied" : "Copy code")
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
    }

    private var displayLanguage: String {
        if let language, !language.isEmpty { return language }
        return "code"
    }

    private var codeBackground: Color {
        Color.black.opacity(0.35)
    }

    private func copy() {
        UIPasteboard.general.string = code
        if reduceMotion {
            copied = true
        } else {
            withAnimation(.easeInOut(duration: 0.15)) { copied = true }
        }
        // Reset the affordance after a moment.
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.6) {
            if reduceMotion {
                copied = false
            } else {
                withAnimation(.easeInOut(duration: 0.2)) { copied = false }
            }
        }
    }
}
