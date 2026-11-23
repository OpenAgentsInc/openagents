// Firm reason: 6 (macOS SwiftUI @main entry point with pure-black OLED main window, MenuBarExtra status companion, and keyboard shortcuts).
import SwiftUI

@main
struct OpenAgentsMacApp: App {
    @StateObject private var store = AntigravityStore()
    @StateObject private var hostServer = TailscaleHostServer()
    @State private var showCommandPalette = false
    @State private var showInspector = false
    @Environment(\.openWindow) private var openWindow

    var body: some Scene {
        WindowGroup("OpenAgents", id: "main") {
            MacRootView(
                store: store,
                hostServer: hostServer,
                showCommandPalette: $showCommandPalette,
                showInspector: $showInspector
            )
            .preferredColorScheme(.dark)
            .frame(minWidth: 880, minHeight: 580)
            .onAppear {
                store.onForeground()
                hostServer.start()
            }
            .onOpenURL { url in
                store.handleIncomingURL(url)
            }
        }
        .windowStyle(.hiddenTitleBar)
        .commands {
            CommandGroup(replacing: .newItem) {
                Button("New Antigravity Chat") {
                    store.startNewChat()
                }
                .keyboardShortcut("n", modifiers: .command)
            }
            CommandMenu("Antigravity") {
                Button("Command Palette…") {
                    showCommandPalette.toggle()
                }
                .keyboardShortcut("k", modifiers: .command)

                Button(showInspector ? "Hide Workspace Inspector" : "Show Workspace Inspector") {
                    showInspector.toggle()
                }
                .keyboardShortcut("i", modifiers: [.command, .shift])

                Divider()

                Button("Host & Tailscale Computers…") {
                    store.isComputersPresented = true
                }
                .keyboardShortcut(",", modifiers: .command)

                Button("Refresh Active Session") {
                    Task { await store.refreshActiveComputer() }
                }
                .keyboardShortcut("r", modifiers: .command)
            }
        }

        MenuBarExtra {
            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 6) {
                    Circle()
                        .fill(hostServer.isListening && hostServer.daemonRunning ? OATheme.statusReady : OATheme.statusWarn)
                        .frame(width: 7, height: 7)
                    Text(hostServer.isListening ? "Tailscale Host Listening (:\(hostServer.listenPort))" : "Tailscale Host Offline")
                        .font(.system(size: 12, weight: .semibold))
                }
                if let owner = hostServer.owner {
                    Text(owner.dns.isEmpty ? owner.ip : owner.dns)
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(.secondary)
                }
                Text("Connected clients: \(hostServer.activeClients) · Working chats: \(store.workingSessions.count)")
                    .font(.system(size: 11))
                    .foregroundStyle(.secondary)

                Divider()

                Button("Open OpenAgents Window") {
                    openWindow(id: "main")
                    NSApp.activate(ignoringOtherApps: true)
                }
                Button("New Antigravity Chat") {
                    openWindow(id: "main")
                    NSApp.activate(ignoringOtherApps: true)
                    store.startNewChat()
                }
                if !hostServer.daemonRunning {
                    Button("Start Local Host Daemon") {
                        hostServer.startLocalDaemon()
                    }
                }
                Divider()
                Button("Quit OpenAgents") {
                    hostServer.stop()
                    NSApp.terminate(nil)
                }
            }
            .padding(10)
            .frame(width: 260)
        } label: {
            Label("OpenAgents", systemImage: store.workingSessions.isEmpty ? "sparkles" : "bolt.circle.fill")
        }
        .menuBarExtraStyle(.window)
    }
}
