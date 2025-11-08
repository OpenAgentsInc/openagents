import SwiftUI
import OpenAgentsCore
import NostrSDK
#if os(macOS)
import AppKit
#endif

#if os(macOS)
#if DEBUG
struct NostrEventCard: View {
    let item: NostrEventFeedManager.DVMEventItem
    let feedManager: NostrEventFeedManager
    @State private var isExpanded = false

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            compactView

            Button(isExpanded ? "Show less" : "Show more") {
                isExpanded.toggle()
            }
            .buttonStyle(.plain)
            .font(OAFonts.mono(.body, 12))

            if isExpanded {
                eventDetail
            }
        }
        .padding(12)
        .background(RoundedRectangle(cornerRadius: 8, style: .continuous).fill(OATheme.Colors.border.opacity(0.25)))
    }

    private var compactView: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 8) {
                typeBadge
                Text(kindLabel)
                    .font(OAFonts.ui(.body, 14))
                    .foregroundStyle(OATheme.Colors.textPrimary)
                Spacer()
                Text(relTime)
                    .font(OAFonts.ui(.caption, 11))
                    .foregroundStyle(OATheme.Colors.textSecondary)
            }

            HStack(spacing: 10) {
                labeledValue("ID", shortHex(item.id)) {
                    copy(item.id)
                }
                labeledValue("Author", shortNpub(item.authorNpub)) {
                    copy(item.authorNpub)
                }
            }
            .font(OAFonts.ui(.caption, 11))

            contentPreview
            tagSummary
        }
    }

    private var eventDetail: some View {
        VStack(alignment: .leading, spacing: 8) {
            Divider()
            HStack(spacing: 16) {
                labeledValue("Event ID", item.event.id) { copy(item.event.id) }
                labeledValue("Pubkey", item.event.pubkey) { copy(item.event.pubkey) }
            }
            .font(OAFonts.mono(.body, 11))
            HStack(spacing: 16) {
                labeledValue("Kind", String(item.event.kind.rawValue)) {}
                labeledValue("Created", DateFormatter.localizedString(from: item.timestamp, dateStyle: .medium, timeStyle: .medium)) {}
            }
            HStack(spacing: 16) {
                let isValid = feedManager.verifySignature(item.event)
                Image(systemName: isValid ? "checkmark.seal" : "xmark.seal")
                    .foregroundStyle(isValid ? OATheme.Colors.success : OATheme.Colors.danger)
                Text(isValid ? "Signature valid" : "Signature invalid")
                    .font(OAFonts.ui(.caption, 11))
                    .foregroundStyle(OATheme.Colors.textSecondary)
            }

            // Content block
            VStack(alignment: .leading, spacing: 6) {
                Text("Content")
                    .font(OAFonts.ui(.subheadline, 12))
                    .foregroundStyle(OATheme.Colors.textSecondary)
                ScrollView(.horizontal) {
                    Text(item.event.content)
                        .font(OAFonts.mono(.body, 11))
                        .textSelection(.enabled)
                        .lineLimit(8)
                }
                Button("Copy Event JSON") { feedManager.copyEventJSON(item.event) }
                    .buttonStyle(.bordered)
                    .font(OAFonts.mono(.body, 12))
            }

            // Tags table
            VStack(alignment: .leading, spacing: 6) {
                Text("Tags (\(item.event.tags.count)):")
                    .font(OAFonts.ui(.subheadline, 12))
                    .foregroundStyle(OATheme.Colors.textSecondary)
                ForEach(Array(item.event.tags.enumerated()), id: \.offset) { _, tag in
                    HStack(spacing: 8) {
                        Text("[\(tag.name)]")
                            .font(OAFonts.mono(.body, 11))
                            .foregroundStyle(OATheme.Colors.textSecondary)
                        Text(tag.value)
                            .font(OAFonts.mono(.body, 11))
                            .foregroundStyle(OATheme.Colors.textPrimary)
                        if !tag.otherParameters.isEmpty {
                            Text(tag.otherParameters.joined(separator: ", "))
                                .font(OAFonts.mono(.body, 11))
                                .foregroundStyle(OATheme.Colors.textTertiary)
                        }
                    }
                }
            }

            Button("View in Explorer") {
                if let note = item.event.bech32NoteId, let url = URL(string: "https://njump.me/\(note)") {
                    NSWorkspace.shared.open(url)
                }
            }
            .buttonStyle(.link)
            .font(OAFonts.mono(.body, 12))
        }
    }

    // MARK: - Small UI helpers
    private var typeBadge: some View {
        let color: Color
        switch item.type {
        case .jobRequest: color = .blue
        case .jobResult: color = .green
        case .jobFeedback: color = .orange
        }
        return Text(badgeText)
            .font(OAFonts.ui(.caption, 11))
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(Capsule().fill(color.opacity(0.2)))
            .foregroundStyle(color)
    }

    private var badgeText: String {
        switch item.type {
        case .jobRequest: return "Request"
        case .jobResult: return "Result"
        case .jobFeedback: return "Feedback"
        }
    }

    private var kindLabel: String {
        if let info = item.kindInfo { return info.displayName }
        return "Kind \(item.event.kind.rawValue)"
    }

    private var relTime: String {
        let seconds = Int(Date().timeIntervalSince(item.timestamp))
        if seconds < 10 { return "Just now" }
        if seconds < 60 { return "\(seconds)s ago" }
        let minutes = seconds / 60
        if minutes < 60 { return "\(minutes)m ago" }
        let hours = minutes / 60
        if hours < 24 { return "\(hours)h ago" }
        let days = hours / 24
        return "\(days)d ago"
    }

    private var contentPreview: some View {
        let content = item.event.content
        let preview: String
        if content.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            preview = "[No content]"
        } else if content.first == "{" || content.first == "[" {
            preview = "[JSON content]"
        } else if content.contains("?iv=") {
            preview = "[Encrypted content (NIP-04)]"
        } else {
            preview = content
        }
        return Text(preview)
            .font(OAFonts.ui(.caption, 11))
            .foregroundStyle(OATheme.Colors.textSecondary)
            .lineLimit(2)
    }

    private var tagSummary: some View {
        let names: [String] = Array(Set(item.event.tags.map { $0.name }))
        let first = names.prefix(5)
        let more = names.count > 5 ? " +\(names.count - 5)" : ""
        let label = first.joined(separator: ", ") + more
        return Text("Tags: \(label)")
            .font(OAFonts.ui(.caption, 11))
            .foregroundStyle(OATheme.Colors.textTertiary)
    }

    private func labeledValue(_ label: String, _ value: String, action: @escaping () -> Void) -> some View {
        HStack(spacing: 6) {
            Text("\(label):")
                .fontWeight(.semibold)
                .foregroundStyle(OATheme.Colors.textSecondary)
            Text(value)
                .textSelection(.enabled)
                .foregroundStyle(OATheme.Colors.textPrimary)
            Button(action: action) { Image(systemName: "doc.on.doc") }
                .buttonStyle(.plain)
                .help("Copy")
        }
    }

    private func shortHex(_ id: String) -> String {
        let pfx = String(id.prefix(8))
        return id.count > 8 ? pfx + "…" : pfx
    }

    private func shortNpub(_ npub: String) -> String {
        let pfx = String(npub.prefix(8))
        return npub.count > 8 ? pfx + "…" : pfx
    }

    private func copy(_ s: String) {
        #if os(macOS)
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(s, forType: .string)
        #endif
    }
}

#endif
#endif
