// Firm reason: 6 (macOS Host & Tailscale Computers modal for managing local coder-host daemon, LaunchAgent, Tailscale port 47109 listener, and remote Tailscale host pairing).
import AppKit
import SwiftUI

struct MacHostSettingsView: View {
    @ObservedObject var store: AntigravityStore
    @ObservedObject var hostServer: TailscaleHostServer
    @Environment(\.dismiss) private var dismiss

    @State private var remoteAddress = ""
    @State private var remoteName = ""
    @State private var remotePort = String(AppTailnet.port)
    @State private var remoteError: String?
    @State private var isAdding = false
    @State private var copiedKey = false

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Host & Tailscale Computers")
                    .font(.system(size: 16, weight: .semibold))
                    .foregroundStyle(OATheme.textPrimary)
                Spacer()
                Button("Done") { dismiss() }
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(Color.black)
                    .padding(.horizontal, 14)
                    .padding(.vertical, 6)
                    .background(Color.white, in: Capsule())
                    .buttonStyle(.plain)
            }
            .padding(16)

            Divider().overlay(OATheme.hairline)

            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    localHostSection
                    remoteHostsSection
                    identitySection
                }
                .padding(18)
            }
        }
        .frame(width: 580, height: 620)
        .background(OATheme.background)
        .onAppear { hostServer.refreshEnvironment() }
    }

    // MARK: - This Mac Host Server

    private var localHostSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("THIS MAC (TAILSCALE HOST SERVER)")
                .font(.system(size: 11, weight: .semibold))
                .tracking(0.6)
                .foregroundStyle(OATheme.textMuted)

            VStack(alignment: .leading, spacing: 10) {
                statusRow(
                    title: "Tailscale Account",
                    value: hostServer.owner.map { "\($0.login.isEmpty ? $0.name : $0.login) · \($0.dns.isEmpty ? $0.ip : $0.dns)" } ?? "Not running",
                    ok: hostServer.owner != nil
                )
                statusRow(
                    title: "Direct Listener (Port \(hostServer.listenPort))",
                    value: hostServer.isListening ? "Listening (\(hostServer.activeClients) subscribed clients)" : "Stopped",
                    ok: hostServer.isListening
                )
                statusRow(
                    title: "Local Host Daemon (service.sock)",
                    value: hostServer.daemonRunning ? "Running (--no-relay)" : "Not running",
                    ok: hostServer.daemonRunning
                )
                statusRow(
                    title: "Antigravity CLI (agy)",
                    value: hostServer.agyPath ?? "Not found on PATH",
                    ok: hostServer.agyPath != nil
                )

                Divider().overlay(OATheme.hairline)

                HStack {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Served Workspace Root")
                            .font(.system(size: 12, weight: .medium))
                            .foregroundStyle(OATheme.textPrimary)
                        Text(hostServer.workspacePath)
                            .font(.system(size: 11, design: .monospaced))
                            .foregroundStyle(OATheme.textSecondary)
                            .lineLimit(1)
                            .truncationMode(.middle)
                    }
                    Spacer()
                    Button("Choose…") {
                        let panel = NSOpenPanel()
                        panel.canChooseDirectories = true
                        panel.canChooseFiles = false
                        if panel.runModal() == .OK, let url = panel.url {
                            hostServer.setWorkspacePath(url.path)
                        }
                    }
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(OATheme.textPrimary)
                    .padding(.horizontal, 10)
                    .padding(.vertical, 5)
                    .background(OATheme.elevated, in: Capsule())
                    .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
                    .buttonStyle(.plain)
                }

                HStack(spacing: 8) {
                    if !hostServer.daemonRunning {
                        Button("Start Daemon Now") {
                            hostServer.startLocalDaemon()
                        }
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundStyle(Color.black)
                        .padding(.horizontal, 12)
                        .padding(.vertical, 6)
                        .background(Color.white, in: Capsule())
                        .buttonStyle(.plain)
                    }

                    Button(hostServer.launchAgentInstalled ? "Remove Background LaunchAgent" : "Install Background LaunchAgent") {
                        if hostServer.launchAgentInstalled {
                            hostServer.removeLaunchAgent()
                        } else {
                            hostServer.installLaunchAgent()
                        }
                    }
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(OATheme.textPrimary)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 6)
                    .background(OATheme.elevated, in: Capsule())
                    .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
                    .buttonStyle(.plain)

                    Spacer()

                    Button("Refresh") {
                        hostServer.refreshEnvironment()
                        store.reloadComputers()
                    }
                    .font(.system(size: 12))
                    .foregroundStyle(OATheme.textSecondary)
                    .buttonStyle(.plain)
                }

                Text(hostServer.statusNote)
                    .font(.system(size: 11))
                    .foregroundStyle(OATheme.textMuted)
            }
            .padding(14)
            .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
        }
    }

    private func statusRow(title: String, value: String, ok: Bool) -> some View {
        HStack {
            Circle()
                .fill(ok ? OATheme.statusReady : OATheme.statusWarn)
                .frame(width: 7, height: 7)
            Text(title)
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(OATheme.textPrimary)
            Spacer()
            Text(value)
                .font(.system(size: 11, design: .monospaced))
                .foregroundStyle(OATheme.textSecondary)
                .lineLimit(1)
        }
    }

    // MARK: - Remote Tailscale Computers

    private var remoteHostsSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("REMOTE TAILSCALE COMPUTERS")
                .font(.system(size: 11, weight: .semibold))
                .tracking(0.6)
                .foregroundStyle(OATheme.textMuted)

            let remotes = store.computers.filter { !$0.isLocalMac }
            if !remotes.isEmpty {
                VStack(spacing: 8) {
                    ForEach(remotes) { comp in
                        HStack {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(comp.name)
                                    .font(.system(size: 13, weight: .semibold))
                                    .foregroundStyle(OATheme.textPrimary)
                                Text("\(comp.displayAddress) · \(comp.reachability.label)")
                                    .font(.system(size: 11, design: .monospaced))
                                    .foregroundStyle(OATheme.textSecondary)
                            }
                            Spacer()
                            if comp.paired == nil || comp.paired?.isExpired == true {
                                Button("Pair over Tailscale") {
                                    Task { remoteError = await store.pairOverTailscale(comp) }
                                }
                                .font(.system(size: 11, weight: .semibold))
                                .foregroundStyle(Color.black)
                                .padding(.horizontal, 10)
                                .padding(.vertical, 5)
                                .background(Color.white, in: Capsule())
                                .buttonStyle(.plain)
                            } else {
                                Button("Select") {
                                    store.selectComputer(comp)
                                    dismiss()
                                }
                                .font(.system(size: 11, weight: .medium))
                                .foregroundStyle(OATheme.textPrimary)
                                .padding(.horizontal, 10)
                                .padding(.vertical, 5)
                                .background(OATheme.elevated, in: Capsule())
                                .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
                                .buttonStyle(.plain)
                            }
                            Button {
                                store.forgetComputer(comp)
                            } label: {
                                Image(systemName: "trash").font(.system(size: 11)).foregroundStyle(OATheme.statusError)
                            }
                            .buttonStyle(.plain)
                        }
                        .padding(10)
                        .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                        .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                    }
                }
            }

            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 8) {
                    TextField("mac-studio.tailnet.ts.net or 100.x.y.z", text: $remoteAddress)
                        .textFieldStyle(.plain)
                        .font(.system(size: 12, design: .monospaced))
                        .foregroundStyle(OATheme.textPrimary)
                        .padding(8)
                        .background(OATheme.elevated, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
                        .overlay(RoundedRectangle(cornerRadius: 7, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))

                    TextField("Name", text: $remoteName)
                        .textFieldStyle(.plain)
                        .font(.system(size: 12))
                        .foregroundStyle(OATheme.textPrimary)
                        .padding(8)
                        .frame(width: 130)
                        .background(OATheme.elevated, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
                        .overlay(RoundedRectangle(cornerRadius: 7, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))

                    TextField("47109", text: $remotePort)
                        .textFieldStyle(.plain)
                        .font(.system(size: 12, design: .monospaced))
                        .foregroundStyle(OATheme.textPrimary)
                        .padding(8)
                        .frame(width: 68)
                        .background(OATheme.elevated, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
                        .overlay(RoundedRectangle(cornerRadius: 7, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                }

                if let remoteError {
                    Text(remoteError)
                        .font(.system(size: 11))
                        .foregroundStyle(OATheme.statusError)
                }

                Button(isAdding ? "Pairing…" : "Add & Pair over Tailscale") {
                    Task {
                        isAdding = true
                        remoteError = nil
                        let port = UInt16(remotePort.trimmingCharacters(in: .whitespaces)) ?? AppTailnet.port
                        if let err = await store.addManualComputer(address: remoteAddress, name: remoteName, port: port, autoPair: true) {
                            remoteError = err
                        } else {
                            remoteAddress = ""
                            remoteName = ""
                        }
                        isAdding = false
                    }
                }
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(Color.black)
                .padding(.horizontal, 14)
                .padding(.vertical, 7)
                .background(Color.white, in: Capsule())
                .buttonStyle(.plain)
                .disabled(remoteAddress.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || isAdding)
            }
            .padding(12)
            .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
        }
    }

    // MARK: - Device Identity

    private var identitySection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("THIS MAC'S CLIENT DEVICE KEY (~/.openagents/macos)")
                .font(.system(size: 11, weight: .semibold))
                .tracking(0.6)
                .foregroundStyle(OATheme.textMuted)

            HStack {
                Text(store.devicePublicKeyHex.isEmpty ? "Unavailable" : store.devicePublicKeyHex)
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(OATheme.textSecondary)
                    .lineLimit(1)
                    .truncationMode(.middle)
                Spacer()
                Button(copiedKey ? "Copied" : "Copy") {
                    OAClipboard.copy(store.devicePublicKeyHex)
                    copiedKey = true
                    DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) { copiedKey = false }
                }
                .font(.system(size: 11, weight: .medium))
                .foregroundStyle(OATheme.textPrimary)
                .buttonStyle(.plain)
            }
            .padding(12)
            .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
        }
    }
}
