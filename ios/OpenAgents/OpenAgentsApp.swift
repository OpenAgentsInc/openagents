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
                .onAppear { _ = BerkeleyFont.registerAll() }
                .environment(\.font, BerkeleyFont.font(relativeTo: .body, size: 16))
                .tint(OATheme.Colors.accent)
        }
        .modelContainer(sharedModelContainer)
    }
}
