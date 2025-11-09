//
//  OpenAgentsApp.swift
//  OpenAgents
//
//  Created by Christopher David on 11/3/25.
//

import SwiftUI
import OSLog
import SwiftData
import OpenAgentsCore

@main
struct OpenAgentsApp: App {
    #if os(iOS)
    @UIApplicationDelegateAdaptor(AppDelegate.self) var appDelegate
    #endif
    @StateObject private var bridge = BridgeManager()
    #if os(macOS)
    @StateObject private var tinyvex = TinyvexManager()
    #endif
    var sharedModelContainer: ModelContainer = {
        let schema = Schema([
            Item.self,
        ])
        let modelConfiguration = ModelConfiguration(schema: schema, isStoredInMemoryOnly: false)

        do {
            return try ModelContainer(for: schema, configurations: [modelConfiguration])
        } catch {
            OpenAgentsLog.app.error("Could not create persistent ModelContainer: \(error.localizedDescription)")
            // Fall back to in-memory store to keep the app running
            let fallback = ModelConfiguration(schema: schema, isStoredInMemoryOnly: true)
            if let mc = try? ModelContainer(for: schema, configurations: [fallback]) {
                return mc
            }
            // If even this fails, treat as a critical configuration error
            OpenAgentsLog.app.fault("ModelContainer creation failed for both persistent and in-memory configurations.")
            preconditionFailure("ModelContainer initialization failed")
        }
    }()

    init() {
        let ts = ISO8601DateFormatter().string(from: Date())
        OpenAgentsLog.app.info("OpenAgentsApp init at \(ts)")
    }

    var body: some Scene {
        WindowGroup {
            Group {
                #if os(iOS)
                ChatHomeView()
                #else
                ChatMacOSView()
                #endif
            }
            .onAppear {
                // Fonts registered at launch in AppDelegate; only warm up here
                #if os(iOS)
                PerformanceWarmup.preloadMonoFont()
                PerformanceWarmup.prewarmHaptics()
                PerformanceWarmup.prewarmKeyboardAndTextInput()
                PerformanceWarmup.prewarmResponderSilently()
                #else
                MacWindowUtils.fitToScreen(margin: 24)
                #endif
            }
            .environment(\.font, OAFonts.ui(.body, 16))
            .tint(OATheme.Colors.accent)
            .task {
                let ts = ISO8601DateFormatter().string(from: Date())
                OpenAgentsLog.app.info("OpenAgentsApp appear; starting bridge at \(ts)")
                bridge.start()
                #if os(macOS)
                tinyvex.start()
                #endif
            }
            .environmentObject(bridge)
            #if os(macOS)
            .environmentObject(tinyvex)
            #endif
        }
        .modelContainer(sharedModelContainer)
        #if os(macOS)
        .windowStyle(.titleBar)
        .windowResizability(.contentSize)
        #endif
    }
}
