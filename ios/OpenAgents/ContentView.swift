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
            // Only overlay our darker glass header when no thread is selected
            if selectedURL == nil {
                GlassHeader(title: "OpenAgents")
                    .allowsHitTesting(false)
            }
        }
        .background(OATheme.Colors.background.ignoresSafeArea())
        .preferredColorScheme(.dark)
    }
}

#Preview {
    ContentView()
}
