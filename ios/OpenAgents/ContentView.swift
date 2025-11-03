//
//  ContentView.swift
//  OpenAgents
//
//  Created by Christopher David on 11/3/25.
//

import SwiftUI
import SwiftData
#if canImport(OpenAgentsCore)
import OpenAgentsCore
#endif

struct ContentView: View {
    var body: some View {
        NavigationSplitView {
            HistorySidebar()
        } detail: {
            Text("Select a thread")
                .font(.headline)
                .foregroundStyle(.secondary)
        }
    }
}

#Preview {
    ContentView()
}
