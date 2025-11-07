import SwiftUI

#if os(macOS)

struct BridgeSetupInstructionsSheet: View {
    @Environment(\.dismiss) var dismiss
    @EnvironmentObject var bridge: BridgeManager

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 24) {
                    // Header
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Bridge Setup")
                            .font(OAFonts.ui(.title, 24))
                            .foregroundStyle(OATheme.Colors.textPrimary)

                        Text("Connect your iPhone or iPad to this Mac")
                            .font(OAFonts.ui(.body, 15))
                            .foregroundStyle(OATheme.Colors.textSecondary)
                    }

                    Divider()

                    // Status Section
                    VStack(alignment: .leading, spacing: 12) {
                        Label("Current Status", systemImage: "info.circle")
                            .font(OAFonts.ui(.headline, 16))
                            .foregroundStyle(OATheme.Colors.textPrimary)

                        HStack(spacing: 12) {
                            Circle()
                                .fill(statusColor)
                                .frame(width: 12, height: 12)

                            VStack(alignment: .leading, spacing: 4) {
                                Text(statusText)
                                    .font(OAFonts.ui(.body, 14))
                                    .foregroundStyle(OATheme.Colors.textPrimary)

                                if let port = serverPort {
                                    Text("Port: \(String(port))")
                                        .font(OAFonts.ui(.caption, 12))
                                        .foregroundStyle(OATheme.Colors.textSecondary)
                                }
                            }
                        }
                        .padding(16)
                        .background(
                            RoundedRectangle(cornerRadius: 12, style: .continuous)
                                .fill(Color.black.opacity(0.2))
                        )
                    }

                    Divider()

                    // Instructions
                    VStack(alignment: .leading, spacing: 16) {
                        Label("Setup Instructions", systemImage: "list.number")
                            .font(OAFonts.ui(.headline, 16))
                            .foregroundStyle(OATheme.Colors.textPrimary)

                        InstructionStep(
                            number: 1,
                            title: "Ensure Bridge is Running",
                            description: "The bridge server should start automatically when the app launches. Check the status above."
                        )

                        InstructionStep(
                            number: 2,
                            title: "Connect Devices to Same Network",
                            description: "Make sure your iPhone/iPad and this Mac are on the same Wi-Fi network."
                        )

                        InstructionStep(
                            number: 3,
                            title: "Open OpenAgents on iOS",
                            description: "Launch the OpenAgents app on your iPhone or iPad. It will automatically discover this Mac."
                        )

                        InstructionStep(
                            number: 4,
                            title: "Manual Connection (Optional)",
                            description: "If auto-discovery fails, tap the connection button and enter this Mac's IP address and port \(serverPort ?? 8080)."
                        )
                    }

                    Divider()

                    // Troubleshooting
                    VStack(alignment: .leading, spacing: 12) {
                        Label("Troubleshooting", systemImage: "wrench.and.screwdriver")
                            .font(OAFonts.ui(.headline, 16))
                            .foregroundStyle(OATheme.Colors.textPrimary)

                        VStack(alignment: .leading, spacing: 8) {
                            Text("• Check firewall settings to allow incoming connections")
                            Text("• Verify both devices are on the same network")
                            Text("• Try restarting the bridge server")
                            Text("• Check the logs for detailed error messages")
                        }
                        .font(OAFonts.ui(.caption, 13))
                        .foregroundStyle(OATheme.Colors.textSecondary)
                    }

                    Spacer(minLength: 20)
                }
                .padding(24)
            }
            .background(OATheme.Colors.background)
            .navigationTitle("Setup Instructions")
            #if os(iOS)
            .navigationBarTitleDisplayMode(.inline)
            #endif
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { dismiss() }
                }
            }
        }
        .preferredColorScheme(.dark)
    }

    private var statusColor: Color {
        switch bridge.status {
        case .connected: return OATheme.Colors.success
        case .advertising: return .blue.opacity(0.8)
        case .connecting, .handshaking: return .yellow.opacity(0.8)
        case .error: return OATheme.Colors.danger
        default: return OATheme.Colors.textTertiary
        }
    }

    private var statusText: String {
        switch bridge.status {
        case .idle: return "Bridge is idle"
        case .advertising(let port):
            let suffix = bridge.connectedClientCount > 0 ? " (\(bridge.connectedClientCount) client\(bridge.connectedClientCount == 1 ? "" : "s"))" : ""
            return "Bridge is advertising on port \(String(port))" + suffix
        case .discovering: return "Discovering..."
        case .connecting(let h, let p): return "Connecting to \(h):\(String(p))"
        case .handshaking(let h, let p): return "Handshaking with \(h):\(String(p))"
        case .connected(let h, let p): return "Connected to \(h):\(String(p))"
        case .error(let e): return "Error: \(e)"
        }
    }

    private var serverPort: UInt16? {
        if case .advertising(let port) = bridge.status {
            return port
        }
        return nil
    }
}

struct InstructionStep: View {
    let number: Int
    let title: String
    let description: String

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            Text("\(number)")
                .font(OAFonts.ui(.headline, 16))
                .foregroundStyle(.white)
                .frame(width: 28, height: 28)
                .background(
                    Circle().fill(OATheme.Colors.accent)
                )

            VStack(alignment: .leading, spacing: 4) {
                Text(title)
                    .font(OAFonts.ui(.body, 14))
                    .fontWeight(.semibold)
                    .foregroundStyle(OATheme.Colors.textPrimary)

                Text(description)
                    .font(OAFonts.ui(.caption, 13))
                    .foregroundStyle(OATheme.Colors.textSecondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

#Preview {
    BridgeSetupInstructionsSheet()
        .environmentObject(BridgeManager())
}

#endif
