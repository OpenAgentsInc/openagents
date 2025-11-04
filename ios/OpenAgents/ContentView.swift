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
                if let row = selectedRow, let u = selectedURL {
                    VStack(spacing: 0) {
                        ThreadHeaderView(row: row, url: u)
                        AcpThreadView(url: selectedURL)
                    }
                } else {
                    AcpThreadView(url: selectedURL)
                }
            }
            // Show our dark glass header ONLY when there is no selected thread
            if selectedURL == nil {
                GlassHeader(title: "OpenAgents")
                    .allowsHitTesting(false)
            }
        }
        .background(OATheme.Colors.background.ignoresSafeArea())
        .task { FMProbe.logAvailability() }
        #if os(iOS)
        .toolbar(.hidden, for: .navigationBar)
        .toolbarBackground(.hidden, for: .navigationBar)
        #endif
        #if os(macOS)
        .toolbar(.hidden, for: .windowToolbar)
        #endif
        .preferredColorScheme(.dark)
    }
}

#Preview {
    ContentView()
}
