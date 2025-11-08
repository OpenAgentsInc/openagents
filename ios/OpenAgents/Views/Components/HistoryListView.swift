import SwiftUI
import OpenAgentsCore

/// Reusable list view for session history items used by HistorySidebar.
struct HistoryListView: View {
    let items: [(LocalThreadSummary, URL?)]
    let selected: LocalThreadSummary?
    var limit: Int = 10
    var titleFor: (LocalThreadSummary) -> String?
    var onSelect: ((LocalThreadSummary, URL?) -> Void)? = nil

    @EnvironmentObject private var bridge: BridgeManager

    var body: some View {
        ForEach(Array(items.prefix(limit).enumerated()), id: \.0) { _, pair in
            HistorySessionRow(
                row: pair.0,
                url: pair.1,
                isActive: isActive(pair.0),
                title: titleFor(pair.0),
                onSelect: onSelect
            )
        }
    }

    private func isActive(_ row: LocalThreadSummary) -> Bool {
        return selected?.id == row.id && selected?.source == row.source
    }
}

/// Single session row item extracted from HistorySidebar
struct HistorySessionRow: View {
    let row: LocalThreadSummary
    let url: URL?
    let isActive: Bool
    let title: String?
    var onSelect: ((LocalThreadSummary, URL?) -> Void)? = nil

    @EnvironmentObject private var bridge: BridgeManager
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        Button(action: {
            #if os(iOS)
            if row.source == "tinyvex" {
                bridge.loadSessionTimeline(sessionId: row.id)
            }
            #endif
            onSelect?(row, url)
        }) {
            VStack(alignment: .leading, spacing: 3) {
                Text(title ?? "Thread")
                    .font(OAFonts.ui(.body, 13))
                    .fontWeight(isActive ? .semibold : .regular)
                    .foregroundStyle(OATheme.Colors.textPrimary)
                    .lineLimit(1)
                HStack(spacing: 8) {
                    if let ts = (row.last_message_ts ?? row.updated_at) as Int64? {
                        Label(relative(ts), systemImage: "clock")
                            .symbolRenderingMode(.monochrome)
                            .font(OAFonts.ui(.caption2, 10))
                            .foregroundStyle(OATheme.Colors.textTertiary)
                    }
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .listRowBackground(isActive ? OATheme.Colors.selection : Color.clear)
        .contentShape(Rectangle())
        .listRowInsets(EdgeInsets(top: 6, leading: 0, bottom: 6, trailing: 0))
        #if os(iOS)
        .listRowSeparator(.hidden)
        #endif
    }

    private func relative(_ ms: Int64) -> String {
        let now = Int64(Date().timeIntervalSince1970 * 1000)
        let diff = max(0, now - ms)
        let sec = diff / 1000
        if sec < 5 { return "just now" }
        if sec < 60 { return "\(sec)s ago" }
        let min = sec / 60
        if min < 60 { return "\(min)m ago" }
        let hr = min / 60
        if hr < 24 { return "\(hr)h ago" }
        let day = hr / 24
        if day < 7 { return "\(day)d ago" }
        let week = day / 7
        return "\(week)w ago"
    }
}

