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
            // Backdrop header provides a blackish glass surface under the toolbar
            GlassHeader(title: "OpenAgents")
                .allowsHitTesting(false)

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
        }
        .background(OATheme.Colors.background.ignoresSafeArea())
        .task { FMProbe.logAvailability() }
        .preferredColorScheme(.dark)
    }
}

#Preview {
    ContentView()
}
