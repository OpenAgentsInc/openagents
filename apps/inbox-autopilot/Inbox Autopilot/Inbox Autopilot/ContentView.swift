import Foundation
import SwiftUI

private enum AppSection: String, CaseIterable, Identifiable {
    case home
    case inbox
    case approvals
    case settings
    case audit

    var id: String { rawValue }

    var title: String {
        switch self {
        case .home:
            return "Home"
        case .inbox:
            return "Inbox"
        case .approvals:
            return "Approvals"
        case .settings:
            return "Settings"
        case .audit:
            return "Audit"
        }
    }

    var icon: String {
        switch self {
        case .home:
            return "sparkles.rectangle.stack"
        case .inbox:
            return "tray.full"
        case .approvals:
            return "checkmark.seal"
        case .settings:
            return "gear"
        case .audit:
            return "clock.arrow.circlepath"
        }
    }
}

struct ContentView: View {
    @EnvironmentObject private var model: AppModel
    @State private var selectedSection: AppSection? = .home

    var body: some View {
        NavigationSplitView {
            List(AppSection.allCases, selection: $selectedSection) { section in
                Label(section.title, systemImage: section.icon)
                    .tag(section)
            }
            .navigationTitle("Inbox Autopilot")
            .toolbar {
                ToolbarItem(placement: .primaryAction) {
                    Button {
                        Task { await model.refreshEverything() }
                    } label: {
                        Label("Refresh", systemImage: "arrow.clockwise")
                    }
                }
            }
        } detail: {
            VStack(alignment: .leading, spacing: 0) {
                statusStrip

                Divider()

                Group {
                    switch selectedSection {
                    case .home:
                        HomeSectionView(onOpenInbox: { selectedSection = .inbox })
                    case .inbox:
                        InboxSectionView(
                            onOpenApprovals: { selectedSection = .approvals },
                            onOpenAudit: { selectedSection = .audit }
                        )
                    case .approvals:
                        ApprovalsSectionView(onOpenThread: { threadID in
                            selectedSection = .inbox
                            Task { await model.openThread(id: threadID) }
                        })
                    case .settings:
                        SettingsSectionView()
                    case .audit:
                        AuditSectionView()
                    case nil:
                        Text("Select a section")
                            .foregroundStyle(.secondary)
                            .frame(maxWidth: .infinity, maxHeight: .infinity)
                    }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .alert("Error", isPresented: Binding(get: {
            model.errorMessage != nil
        }, set: { isPresented in
            if !isPresented {
                model.errorMessage = nil
            }
        })) {
            Button("OK", role: .cancel) {}
        } message: {
            Text(model.errorMessage ?? "Unknown error")
        }
        .sheet(isPresented: $model.needsOnboarding) {
            OnboardingSheetView()
                .environmentObject(model)
        }
    }

    private var statusStrip: some View {
        HStack(spacing: 12) {
            statusBadge(title: "Daemon", ok: model.daemonConnected)
            statusBadge(title: "Gmail", ok: model.gmailConnected)
            statusBadge(title: "ChatGPT", ok: model.chatGPTConnected)

            Spacer()

            if let notice = model.notice {
                Text(notice)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }

            if model.isBusy {
                ProgressView()
                    .controlSize(.small)
            }
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
    }

    private func statusBadge(title: String, ok: Bool) -> some View {
        HStack(spacing: 6) {
            Circle()
                .fill(ok ? Color.green : Color.orange)
                .frame(width: 8, height: 8)
            Text(title)
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 4)
        .background(Color.gray.opacity(0.12), in: Capsule())
    }
}

private struct HomeSectionView: View {
    @EnvironmentObject private var model: AppModel
    @State private var demoRunning = false
    @State private var attemptedAutoLaunch = false

    let onOpenInbox: () -> Void

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text("Inbox Autopilot")
                    .font(.largeTitle)
                    .fontWeight(.semibold)

                Text("WGPUI canvas spike: launch a Rust-rendered window with the same dark dotted background style used in Autopilot desktop.")
                    .foregroundStyle(.secondary)

                HStack(spacing: 10) {
                    Button(demoRunning ? "WGPUI Demo Running" : "Launch WGPUI Canvas Demo") {
                        do {
                            try WGPUIDemoLauncher.shared.launch()
                            demoRunning = true
                            model.notice = "Launched WGPUI demo window."
                        } catch {
                            model.errorMessage = error.localizedDescription
                        }
                    }
                    .buttonStyle(.borderedProminent)

                    Button("Stop Demo") {
                        WGPUIDemoLauncher.shared.stop()
                        demoRunning = false
                        model.notice = "Stopped WGPUI demo."
                    }
                    .disabled(!demoRunning)

                    Button("Open Classic Inbox UI") {
                        onOpenInbox()
                    }
                }

                GroupBox("What this does") {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Runs `cargo run --bin wgpui_background_demo` from `apps/inbox-autopilot/daemon`.")
                        Text("This is a sidecar spike so the existing SwiftUI app remains fully intact.")
                    }
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.top, 2)
                }
            }
            .padding(20)
            .frame(maxWidth: 900, alignment: .leading)
        }
        .onAppear {
            demoRunning = WGPUIDemoLauncher.shared.isRunning
            if !attemptedAutoLaunch {
                attemptedAutoLaunch = true
                if !demoRunning {
                    do {
                        try WGPUIDemoLauncher.shared.launch()
                        demoRunning = true
                        model.notice = "Launched WGPUI demo window."
                    } catch {
                        model.errorMessage = error.localizedDescription
                    }
                }
            }
        }
    }
}

@MainActor
private final class WGPUIDemoLauncher {
    static let shared = WGPUIDemoLauncher()

    private var process: Process?
    var isRunning: Bool { process?.isRunning == true }

    private init() {}

    func launch() throws {
        if process?.isRunning == true {
            return
        }

        let daemonDir = try Self.resolveDaemonDirectory()
        let process = Process()
        process.currentDirectoryURL = daemonDir
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = ["cargo", "run", "--bin", "wgpui_background_demo"]

        try process.run()
        self.process = process
    }

    func stop() {
        process?.terminate()
        process = nil
    }

    private static func resolveDaemonDirectory() throws -> URL {
        let source = URL(fileURLWithPath: #filePath)
        let appRoot = source
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let daemon = appRoot.appendingPathComponent("daemon", isDirectory: true)
        guard FileManager.default.fileExists(atPath: daemon.path) else {
            throw NSError(
                domain: "InboxAutopilot.WGPUI",
                code: 404,
                userInfo: [NSLocalizedDescriptionKey: "Could not find daemon directory at \(daemon.path)"]
            )
        }
        return daemon
    }
}

private struct OnboardingSheetView: View {
    @EnvironmentObject private var model: AppModel

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("Welcome to Inbox Autopilot")
                .font(.title2)
                .fontWeight(.semibold)

            Text("Connect Gmail, choose privacy mode, and select your default backfill range.")
                .foregroundStyle(.secondary)

            Picker("Privacy mode", selection: $model.settings.privacyMode) {
                ForEach(PrivacyMode.allCases, id: \.self) { mode in
                    Text(mode.title).tag(mode)
                }
            }
            .pickerStyle(.segmented)

            Picker("Backfill range", selection: $model.settings.backfillDays) {
                Text("90 days").tag(90)
                Text("12 months").tag(365)
            }
            .pickerStyle(.segmented)

            HStack(spacing: 8) {
                Button(model.gmailConnected ? "Gmail Connected" : "Connect Gmail") {
                    Task { await model.connectGmail() }
                }
                .disabled(model.gmailConnected)

                SecureField("Optional OpenAI API key", text: $model.chatGPTAPIKeyInput)
                    .textFieldStyle(.roundedBorder)

                Button(model.chatGPTConnected ? "ChatGPT Connected" : "Connect ChatGPT") {
                    Task { await model.connectChatGPT() }
                }
                .disabled(model.chatGPTConnected || model.chatGPTAPIKeyInput.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }

            HStack {
                Button("Save & Continue") {
                    Task {
                        await model.saveSettings()
                        if model.gmailConnected {
                            model.needsOnboarding = false
                        }
                    }
                }
                .keyboardShortcut(.defaultAction)
                .disabled(!model.gmailConnected)

                Button("Skip for now") {
                    model.needsOnboarding = false
                }
            }
        }
        .padding(20)
        .frame(width: 560)
    }
}
