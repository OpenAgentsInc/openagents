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

    var body: some Scene {
        WindowGroup {
            ContentView()
                .onAppear { _ = BerkeleyFont.registerAll(); _ = InterFont.registerAll() }
                .environment(\.font, BerkeleyFont.font(relativeTo: .body, size: 16))
                .tint(OATheme.Colors.accent)
                .task { bridge.start() }
                .environmentObject(bridge)
        }
        .modelContainer(sharedModelContainer)
        #if os(macOS)
        .windowStyle(.hiddenTitleBar)
        #endif
    }
}
