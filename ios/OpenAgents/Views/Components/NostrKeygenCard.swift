import SwiftUI
import OpenAgentsCore

#if os(macOS)
struct NostrKeygenCard: View {
    @State private var keys: NostrKeys? = nil
    @State private var lastError: String? = nil
    @State private var showNsec: Bool = false
    @State private var showPrivHex: Bool = false

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 12) {
                Image(systemName: keys == nil ? "key" : "key.fill")
                Text("Generate Nostr Keypair")
                    .font(OAFonts.ui(.body, 15))
                Spacer()
                Button(action: generate) {
                    Label("Generate", systemImage: "wand.and.stars")
                        .labelStyle(.titleAndIcon)
                        .font(OAFonts.mono(.body, 13))
                }
                .buttonStyle(.borderedProminent)
                .tint(OATheme.Colors.warning)
            }

            if let k = keys {
                Group {
                    // Public first
                    kvRow("npub", k.npub, copyable: true)
                    kvRow("public (hex)", k.publicHex, copyable: true)
                    // Private last
                    secureRow("nsec", k.nsec, isRevealed: $showNsec)
                    secureRow("private (hex)", k.privateHex, isRevealed: $showPrivHex)
                }
                .font(OAFonts.ui(.caption, 11))
                .foregroundStyle(OATheme.Colors.textSecondary)
            }

            if let err = lastError {
                Text(err)
                    .font(OAFonts.ui(.caption, 11))
                    .foregroundStyle(.red)
            }
        }
    }

    private func generate() {
        if let out = NostrSupport.generate() {
            self.keys = out
            self.lastError = nil
        } else {
            self.lastError = "NostrSDK unavailable or key generation failed"
        }
    }

    @ViewBuilder
    private func kvRow(_ label: String, _ value: String, copyable: Bool) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Text("\(label):")
                .fontWeight(.semibold)
            Text(value)
                .textSelection(.enabled)
                .lineLimit(2)
            Spacer()
            if copyable {
                Button(action: { copyToPasteboard(value) }) {
                    Image(systemName: "doc.on.doc")
                }
                .buttonStyle(.plain)
                .help("Copy to clipboard")
            }
        }
    }

    @ViewBuilder
    private func secureRow(_ label: String, _ value: String, isRevealed: Binding<Bool>) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Text("\(label):")
                .fontWeight(.semibold)
            if isRevealed.wrappedValue {
                Text(value)
                    .textSelection(.enabled)
                    .lineLimit(2)
            } else {
                Text(masked(value))
                    .font(OAFonts.mono(.body, 11))
            }
            Spacer()
            Button(action: { isRevealed.wrappedValue.toggle() }) {
                Image(systemName: isRevealed.wrappedValue ? "eye.slash" : "eye")
            }
            .buttonStyle(.plain)
            .help(isRevealed.wrappedValue ? "Hide" : "Reveal")
            if isRevealed.wrappedValue {
                Button(action: { copyToPasteboard(value) }) {
                    Image(systemName: "doc.on.doc")
                }
                .buttonStyle(.plain)
                .help("Copy to clipboard")
            }
        }
    }

    private func masked(_ value: String) -> String {
        // Obscure with the same length to avoid leaking length
        guard !value.isEmpty else { return "" }
        return String(repeating: "•", count: value.count)
    }

    private func copyToPasteboard(_ s: String) {
        #if os(macOS)
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(s, forType: .string)
        #endif
    }
}
#endif
