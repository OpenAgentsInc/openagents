import SwiftUI

struct BridgeStatusChip: View {
    @EnvironmentObject var bridge: BridgeManager

    @State private var showManual = false

    var body: some View {
        HStack(spacing: 8) {
            Circle().fill(color).frame(width: 8, height: 8)
            Text(text)
                .font(OAFonts.ui(.caption2, 11))
                .foregroundStyle(OATheme.Colors.textSecondary)
            Spacer()
            if !bridge.lastLog.isEmpty {
                Text(truncate(bridge.lastLog))
                    .font(OAFonts.ui(.caption2, 10))
                    .foregroundStyle(OATheme.Colors.textTertiary)
                    .lineLimit(1)
            }
            #if os(iOS)
            Button(action: { showManual = true }) {
                Image(systemName: "link.badge.plus")
            }
            .buttonStyle(.borderless)
            .foregroundStyle(OATheme.Colors.textSecondary)
            #endif
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .fill(Color.black.opacity(0.15))
        )
        #if os(iOS)
        .sheet(isPresented: $showManual) {
            let mgr = bridge
            ManualConnectSheet { host, port in
                mgr.log("manual", "Connecting to \(host):\(port)")
                mgr.performManualConnect(host: host, port: port)
            }
        }
        #endif
    }

    private var color: Color {
        switch bridge.status {
        case .connected: return OATheme.Colors.success
        case .connecting, .handshaking: return .yellow.opacity(0.8)
        case .error: return OATheme.Colors.danger
        case .advertising: return .blue.opacity(0.8)
        default: return OATheme.Colors.textTertiary
        }
    }
    private var text: String {
        switch bridge.status {
        case .idle: return "Bridge: idle"
        case .advertising(let port):
            #if os(macOS)
            let suffix = bridge.connectedClientCount > 0 ? " (\(bridge.connectedClientCount) client\(bridge.connectedClientCount == 1 ? "" : "s"))" : ""
            return "Bridge: advertising :\(port)" + suffix
            #else
            return "Bridge: advertising :\(port)"
            #endif
        case .discovering: return "Bridge: discovering"
        case .connecting(let h, let p): return "Bridge: connecting \(h):\(p)"
        case .handshaking(let h, let p): return "Bridge: handshaking \(h):\(p)"
        case .connected(let h, let p): return "Bridge: connected \(h):\(p)"
        case .error(let e): return "Bridge error: \(e)"
        }
    }
    private func truncate(_ s: String, _ max: Int = 60) -> String {
        if s.count <= max { return s }
        return String(s.prefix(max)) + "…"
    }
}
