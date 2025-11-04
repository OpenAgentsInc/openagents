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
        NavigationSplitView {
            HistorySidebar(onSelect: { row, url in
                self.selectedRow = row
                self.selectedURL = url
            })
            .navigationSplitViewColumnWidth(min: 220, ideal: 260)
        } detail: {
            AcpThreadView(url: selectedURL)
        }
    }
}

#Preview {
    ContentView()
}
