import SwiftUI
import OpenAgentsCore

#if os(macOS)
#if DEBUG
struct NostrRelayStatusView: View {
    @ObservedObject var relayManager: NostrRelayManager
    @State private var newRelay: String = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                statusDot
                Text(aggregateText)
                    .font(OAFonts.mono(.caption, 11))
                    .foregroundStyle(OATheme.Colors.textSecondary)
            }
            ForEach(relayManager.relays) { info in
                HStack(spacing: 8) {
                    dot(for: info.status)
                    Text(info.url.absoluteString)
                        .font(OAFonts.mono(.body, 11))
                        .foregroundStyle(OATheme.Colors.textPrimary)
                    Spacer()
                    if let last = info.lastConnected {
                        Text(last, style: .time)
                            .font(OAFonts.mono(.caption, 10))
                            .foregroundStyle(OATheme.Colors.textTertiary)
                    }
                    if case .error(let msg) = info.status {
                        Text(msg)
                            .font(OAFonts.mono(.caption, 10))
                            .foregroundStyle(OATheme.Colors.danger)
                    }
                    Button(role: .destructive) {
                        relayManager.removeRelay(url: info.url)
                    } label: { Image(systemName: "minus.circle") }
                        .buttonStyle(.plain)
                        .font(OAFonts.mono(.body, 12))
                }
            }

            HStack(spacing: 8) {
                TextField("wss://relay.example.com", text: $newRelay)
                    .textFieldStyle(.roundedBorder)
                    .font(OAFonts.mono(.body, 11))
                    .frame(minWidth: 280)
                Button {
                    if let url = URL(string: newRelay) {
                        try? relayManager.addRelay(url: url)
                        newRelay = ""
                    }
                } label: { Label("Add", systemImage: "plus") }
                .buttonStyle(.bordered)
                .font(OAFonts.mono(.body, 12))
            }
        }
    }

    private var aggregateText: String {
        switch relayManager.connectionStatus {
        case .disconnected: return "Disconnected"
        case .connecting: return "Connecting…"
        case .connected(let count): return "Connected (\(count))"
        case .error(let msg): return "Error: \(msg)"
        }
    }

    @ViewBuilder private var statusDot: some View {
        switch relayManager.connectionStatus {
        case .disconnected: Circle().fill(OATheme.Colors.textTertiary).frame(width: 8, height: 8)
        case .connecting: Circle().fill(.yellow).frame(width: 8, height: 8)
        case .connected: Circle().fill(.green).frame(width: 8, height: 8)
        case .error: Circle().fill(.red).frame(width: 8, height: 8)
        }
    }

    @ViewBuilder private func dot(for status: NostrRelayManager.RelayInfo.RelayStatus) -> some View {
        switch status {
        case .disconnected: Circle().fill(OATheme.Colors.textTertiary).frame(width: 6, height: 6)
        case .connecting: Circle().fill(.yellow).frame(width: 6, height: 6)
        case .connected: Circle().fill(.green).frame(width: 6, height: 6)
        case .error: Circle().fill(.red).frame(width: 6, height: 6)
        }
    }
}
#endif
#endif
