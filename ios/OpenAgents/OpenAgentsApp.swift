//
//  OpenAgentsApp.swift
//  OpenAgents
//
//  Created by Christopher David on 11/3/25.
//

import SwiftUI
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
        print("[Bridge][app] OpenAgentsApp init at \(ISO8601DateFormatter().string(from: Date()))")
    }

    var body: some Scene {
        WindowGroup {
            ContentView()
                .onAppear { _ = BerkeleyFont.registerAll(); _ = InterFont.registerAll() }
                .environment(\.font, OAFonts.ui(.body, 16))
                .tint(OATheme.Colors.accent)
                .task {
                    print("[Bridge][app] OpenAgentsApp appear; starting bridge")
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
