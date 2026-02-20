import SwiftUI
import AppKit

@main
struct Inbox_AutopilotApp: App {
    @StateObject private var appModel = AppModel()
    @StateObject private var lockManager = AppLockManager()
    @Environment(\.scenePhase) private var scenePhase

    var body: some Scene {
        WindowGroup {
            Group {
                if lockManager.isUnlocked {
                    ContentView()
                        .environmentObject(appModel)
                } else {
                    LockScreenView()
                        .environmentObject(lockManager)
                }
            }
            .task {
                await lockManager.unlockIfNeeded()
                await appModel.startup()
            }
            .onChange(of: scenePhase) { _, newPhase in
                if newPhase != .active {
                    lockManager.lock()
                }
            }
        }
        .commands {
            CommandMenu("Inbox Autopilot") {
                Button("Refresh Data") {
                    Task { await appModel.refreshEverything() }
                }
                .keyboardShortcut("r", modifiers: [.command, .shift])

                Button("Sync Now") {
                    Task { await appModel.syncNow() }
                }
                .keyboardShortcut("s", modifiers: [.command, .shift])
            }
        }

        MenuBarExtra("Inbox Autopilot", systemImage: appModel.daemonConnected ? "bolt.horizontal.circle.fill" : "bolt.horizontal.circle") {
            VStack(alignment: .leading, spacing: 8) {
                Text(appModel.daemonConnected ? "Daemon connected" : "Daemon disconnected")
                    .font(.caption)
                Text(appModel.gmailConnected ? "Gmail connected" : "Gmail disconnected")
                    .font(.caption)
                Text(appModel.chatGPTConnected ? "ChatGPT connected" : "ChatGPT optional")
                    .font(.caption)

                Divider()

                Button("Open App") {
                    NSApplication.shared.activate(ignoringOtherApps: true)
                }
                Button("Refresh") {
                    Task { await appModel.refreshEverything() }
                }
                Button("Sync now") {
                    Task { await appModel.syncNow() }
                }

                Divider()

                Toggle("Require unlock", isOn: $lockManager.requiresUnlock)
                Button(lockManager.isUnlocked ? "Lock now" : "Unlock now") {
                    if lockManager.isUnlocked {
                        lockManager.lock()
                    } else {
                        Task { await lockManager.authenticate() }
                    }
                }
            }
            .padding(10)
        }
    }
}

private struct LockScreenView: View {
    @EnvironmentObject private var lockManager: AppLockManager

    var body: some View {
        VStack(spacing: 16) {
            Image(systemName: "lock.fill")
                .font(.system(size: 36))
            Text("Inbox Autopilot is locked")
                .font(.headline)
            Button("Unlock") {
                Task { await lockManager.authenticate() }
            }
            .keyboardShortcut(.defaultAction)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}
