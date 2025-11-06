//
//  OpenAgentsApp.swift
//  OpenAgents
//
//  Created by Christopher David on 11/3/25.
//

import SwiftUI
import OSLog
import SwiftData

@main
struct OpenAgentsApp: App {
    #if os(iOS)
    @UIApplicationDelegateAdaptor(AppDelegate.self) var appDelegate
    #endif
    @StateObject private var bridge = BridgeManager()
    var sharedModelContainer: ModelContainer = {
        let schema = Schema([
            Item.self,
        ])
        let modelConfiguration = ModelConfiguration(schema: schema, isStoredInMemoryOnly: false)

        do {
            return try ModelContainer(for: schema, configurations: [modelConfiguration])
        } catch {
            fatalError("Could not create ModelContainer: \(error)")
        }
    }()

    init() {
        let ts = ISO8601DateFormatter().string(from: Date())
        print("[Bridge][app] OpenAgentsApp init at \(ts)")
        if #available(iOS 16.0, macOS 13.0, *) {
            Logger(subsystem: "com.openagents.app", category: "app").log("OpenAgentsApp init at \(ts, privacy: .public)")
        }
    }

    var body: some Scene {
        WindowGroup {
            ContentView()
                .onAppear { _ = BerkeleyFont.registerAll(); _ = InterFont.registerAll() }
                .environment(\.font, OAFonts.ui(.body, 16))
                .tint(OATheme.Colors.accent)
                .task {
                    let ts = ISO8601DateFormatter().string(from: Date())
                    print("[Bridge][app] OpenAgentsApp appear; starting bridge")
                    if #available(iOS 16.0, macOS 13.0, *) {
                        Logger(subsystem: "com.openagents.app", category: "app").log("appear start bridge at \(ts, privacy: .public)")
                    }
                    bridge.start()
                }
                .environmentObject(bridge)
        }
        .modelContainer(sharedModelContainer)
        #if os(macOS)
        .windowStyle(.hiddenTitleBar)
        #endif
    }
}
