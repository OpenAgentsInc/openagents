// Firm reason: 6 (macOS Host & Tailscale Computers modal for managing local coder-host daemon, LaunchAgent, T3-faithful Antigravity CLI/ACP authentication, Tailscale port 47109 listener, and remote Tailscale host pairing).
import AppKit
import SwiftUI

struct MacHostSettingsView: View {
    @ObservedObject var store: AntigravityStore
    @ObservedObject var hostServer: TailscaleHostServer
    @ObservedObject var authController: AntigravityAuthController
    @Environment(\.dismiss) private var dismiss

    @State private var remoteAddress = ""
    @State private var remoteName = ""
    @State private var remotePort = String(AppTailnet.port)
    @State private var remoteError: String?
    @State private var isAdding = false
    @State private var copiedKey = false
    @State private var apiKeyDraft = ""
    @State private var manualCodeDraft = ""

    init(store: AntigravityStore, hostServer: TailscaleHostServer) {
        self.store = store
        self.hostServer = hostServer
        self.authController = hostServer.authController
    }

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
                    antigravityAuthSection
                    remoteHostsSection
                    identitySection
                }
                .padding(18)
            }
        }
        .frame(width: 600, height: 680)
        .background(OATheme.background)
        .onAppear {
            hostServer.refreshEnvironment(probeAuth: true)
        }
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
                    value: hostServer.headlessOwnsTailnet
                        ? "Headless coder-host active (stays online when app quits)"
                        : (hostServer.isListening ? "Listening (\(hostServer.activeClients) subscribed clients)" : "Stopped"),
                    ok: hostServer.isListening
                )
                statusRow(
                    title: "Local Host Daemon (service.sock)",
                    value: hostServer.daemonRunning ? "Running (--no-relay --tailnet-listen)" : "Not running",
                    ok: hostServer.daemonRunning
                )
                statusRow(
                    title: "Antigravity Sign-In",
                    value: authController.summaryLabel,
                    ok: authController.isAuthenticated
                )
                if let meta = store.activeHostMeta {
                    statusRow(
                        title: "Remote / Host Runtime",
                        value: "host v\(meta.hostVersion.isEmpty ? "1" : meta.hostVersion) · agy \(meta.agyVersion.isEmpty ? "unknown" : meta.agyVersion) · \(meta.capabilities.count) capabilities",
                        ok: !meta.agyUpdateAvailable
                    )
                }

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
                            hostServer.setWorkspacePath(url.path, devicePubkey: store.devicePublicKeyHex)
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
                            hostServer.startLocalDaemon(devicePubkey: store.devicePublicKeyHex)
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
                            hostServer.installLaunchAgent(devicePubkey: store.devicePublicKeyHex)
                        }
                    }
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(OATheme.textPrimary)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 6)
                    .background(OATheme.elevated, in: Capsule())
                    .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
                    .buttonStyle(.plain)

                    if let comp = store.activeComputer, let meta = store.hostMetaByID[comp.id], meta.supportsRemoteUpdate {
                        let updating = store.isUpdatingRemoteAgyHostID == comp.id
                        Button(updating ? "Updating Antigravity…" : (meta.agyUpdateAvailable ? "Update Antigravity" : "Sync agy")) {
                            Task {
                                if let err = await store.updateRemoteAntigravity(on: comp) {
                                    remoteError = err
                                }
                            }
                        }
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(meta.agyUpdateAvailable ? Color.black : OATheme.textPrimary)
                        .padding(.horizontal, 12)
                        .padding(.vertical, 6)
                        .background(meta.agyUpdateAvailable ? OATheme.statusWarn : OATheme.elevated, in: Capsule())
                        .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
                        .buttonStyle(.plain)
                        .disabled(updating)
                    }

                    Spacer()

                    Button("Refresh") {
                        hostServer.refreshEnvironment(probeAuth: true)
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

    // MARK: - Antigravity CLI / ACP Authentication (#13995)

    private var antigravityAuthSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("ANTIGRAVITY AUTHENTICATION (VIA OFFICIAL AGY CLI)")
                .font(.system(size: 11, weight: .semibold))
                .tracking(0.6)
                .foregroundStyle(OATheme.textMuted)

            VStack(alignment: .leading, spacing: 10) {
                HStack {
                    Text("Account Mode")
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(OATheme.textPrimary)
                    Spacer()
                    Picker("Account Mode", selection: Binding(
                        get: { authController.selectedMethod },
                        set: { authController.selectMethod($0) }
                    )) {
                        ForEach(AntigravityAuthMethod.allCases) { m in
                            Text(m.label).tag(m)
                        }
                    }
                    .labelsHidden()
                    .frame(width: 240)
                }

                Text(authController.selectedMethod.subtitle)
                    .font(.system(size: 11))
                    .foregroundStyle(OATheme.textSecondary)

                if authController.selectedMethod == .geminiAPIKey {
                    HStack(spacing: 8) {
                        SecureField("AIza… Gemini API Key", text: $apiKeyDraft)
                            .textFieldStyle(.plain)
                            .font(.system(size: 12, design: .monospaced))
                            .foregroundStyle(OATheme.textPrimary)
                            .padding(8)
                            .background(OATheme.elevated, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
                            .overlay(RoundedRectangle(cornerRadius: 7, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                        Button("Save & Verify") {
                            let key = apiKeyDraft
                            apiKeyDraft = ""
                            Task { await authController.saveAPIKeyAndProbe(key) }
                        }
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundStyle(Color.black)
                        .padding(.horizontal, 12)
                        .padding(.vertical, 7)
                        .background(Color.white, in: Capsule())
                        .buttonStyle(.plain)
                        .disabled(apiKeyDraft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || authController.isProbing)
                    }
                } else {
                    HStack(spacing: 8) {
                        Button(authController.isAuthenticating ? "Signing in…" : (authController.isAuthenticated ? "Re-authenticate" : "Sign in with Google")) {
                            authController.startOAuthConnect()
                        }
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundStyle(Color.black)
                        .padding(.horizontal, 12)
                        .padding(.vertical, 6)
                        .background(Color.white, in: Capsule())
                        .buttonStyle(.plain)
                        .disabled(authController.isAuthenticating)

                        if authController.isAuthenticated {
                            Button("Sign out") {
                                Task { await authController.signOut() }
                            }
                            .font(.system(size: 12, weight: .medium))
                            .foregroundStyle(OATheme.statusError)
                            .padding(.horizontal, 12)
                            .padding(.vertical, 6)
                            .background(OATheme.elevated, in: Capsule())
                            .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
                            .buttonStyle(.plain)
                        }

                        Spacer()

                        Button(authController.isProbing ? "Checking…" : "Check Status") {
                            Task { await authController.probe() }
                        }
                        .font(.system(size: 12))
                        .foregroundStyle(OATheme.textSecondary)
                        .buttonStyle(.plain)
                        .disabled(authController.isProbing)
                    }
                }

                if authController.awaitingManualCode || authController.pendingAuthURL != nil {
                    VStack(alignment: .leading, spacing: 6) {
                        if let authURL = authController.pendingAuthURL {
                            HStack(spacing: 8) {
                                Text(authURL)
                                    .font(.system(size: 10, design: .monospaced))
                                    .foregroundStyle(OATheme.textMuted)
                                    .lineLimit(1)
                                    .truncationMode(.middle)
                                Button("Open Browser") {
                                    if let u = URL(string: authURL) { NSWorkspace.shared.open(u) }
                                }
                                .font(.system(size: 11, weight: .medium))
                                .foregroundStyle(OATheme.textPrimary)
                                .buttonStyle(.plain)
                            }
                        }
                        HStack(spacing: 8) {
                            TextField("Paste callback URL or authorization code…", text: $manualCodeDraft)
                                .textFieldStyle(.plain)
                                .font(.system(size: 12, design: .monospaced))
                                .foregroundStyle(OATheme.textPrimary)
                                .padding(7)
                                .background(OATheme.elevated, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
                                .overlay(RoundedRectangle(cornerRadius: 7, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                            Button("Submit Code") {
                                let code = manualCodeDraft
                                manualCodeDraft = ""
                                Task { await authController.submitManualAuthCode(code) }
                            }
                            .font(.system(size: 11, weight: .semibold))
                            .foregroundStyle(Color.black)
                            .padding(.horizontal, 10)
                            .padding(.vertical, 6)
                            .background(Color.white, in: Capsule())
                            .buttonStyle(.plain)
                            .disabled(manualCodeDraft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                        }
                    }
                }

                if let err = authController.errorMessage, !err.isEmpty {
                    Text(err)
                        .font(.system(size: 11))
                        .foregroundStyle(OATheme.statusError)
                } else if !authController.statusMessage.isEmpty {
                    Text(authController.statusMessage)
                        .font(.system(size: 11))
                        .foregroundStyle(OATheme.textMuted)
                }
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
