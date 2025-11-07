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
    }

    var body: some Scene {
        WindowGroup {
            Group {
                #if os(iOS)
                ChatHomeView()
                #else
                ContentView()
                #endif
            }
            .onAppear {
                // Fonts registered at launch in AppDelegate; only warm up here
                PerformanceWarmup.preloadMonoFont()
                PerformanceWarmup.prewarmHaptics()
                PerformanceWarmup.prewarmKeyboardAndTextInput()
            }
            .environment(\.font, OAFonts.ui(.body, 16))
            .tint(OATheme.Colors.accent)
            .task {
                let ts = ISO8601DateFormatter().string(from: Date())
                print("[Bridge][app] OpenAgentsApp appear; starting bridge at \(ts)")
                bridge.start()
            }
            .environmentObject(bridge)
        }
        .modelContainer(sharedModelContainer)
        #if os(macOS)
        .windowStyle(.titleBar)
        .defaultSize(width: 600, height: 800)
        .windowResizability(.contentMinSize)
        #endif
    }
}
