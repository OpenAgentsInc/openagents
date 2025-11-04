//
//  ContentView.swift
//  OpenAgents
//
//  Created by Christopher David on 11/3/25.
//

import SwiftUI
struct ContentView: View {
    @State private var selectedRow: LocalThreadSummary? = nil
    @State private var selectedURL: URL? = nil

    var body: some View {
        ZStack(alignment: .topLeading) {
            NavigationSplitView {
                HistorySidebar(selected: selectedRow, onSelect: { row, url in
                    self.selectedRow = row
                    self.selectedURL = url
                })
                .navigationSplitViewColumnWidth(min: 220, ideal: 260)
            } detail: {
                AcpThreadView(url: selectedURL)
            }
            #if os(iOS)
            .toolbarBackground(.hidden, for: .navigationBar)
            #endif

            // Lightweight Liquid Glass header spanning the window (non-interactive)
            GlassHeader(title: "OpenAgents")
                .allowsHitTesting(false)
        }
        .background(OATheme.Colors.background.ignoresSafeArea())
        .preferredColorScheme(.dark)
    }
}

#Preview {
    ContentView()
}
