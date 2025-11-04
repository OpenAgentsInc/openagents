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
                // Remove custom header to avoid duplication under system toolbar
                AcpThreadView(url: selectedURL)
            }
            // Gradient sits above content but under the toolbar, creating a soft edge behind the title
            TopEdgeGradient()
        }
        .background(OATheme.Colors.background.ignoresSafeArea())
        .task { FMProbe.logAvailability() }
        #if os(iOS)
        .toolbar(.visible, for: .navigationBar)
        .toolbarBackground(.hidden, for: .navigationBar)
        .toolbarColorScheme(.dark, for: .navigationBar)
        .toolbar {
            ToolbarItem(placement: .principal) {
                Text(selectedRowTitle())
                    .font(Font.custom(BerkeleyFont.defaultName(), size: 15, relativeTo: .headline))
                    .foregroundStyle(OATheme.Colors.textPrimary)
                    .lineLimit(1)
            }
        }
        #endif
        #if os(macOS)
        .toolbar(.visible, for: .windowToolbar)
        .toolbarBackground(.hidden, for: .windowToolbar)
        .toolbar {
            ToolbarItem(placement: .principal) {
                Text(selectedRowTitle())
                    .font(Font.custom(BerkeleyFont.defaultName(), size: 14, relativeTo: .headline))
                    .foregroundStyle(OATheme.Colors.textPrimary)
                    .lineLimit(1)
            }
        }
        #endif
        .preferredColorScheme(.dark)
    }

    private func selectedRowTitle() -> String {
        if let r = selectedRow {
            if let t = r.title, !t.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { return t }
            // Fallback to id tail if title missing
            let s = r.id
            if s.count > 16 { return "…" + String(s.suffix(16)) }
            return s.isEmpty ? "Thread" : s
        }
        return "OpenAgents"
    }
}

#Preview {
    ContentView()
}
