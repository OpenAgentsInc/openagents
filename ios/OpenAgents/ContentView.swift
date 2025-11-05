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
    @State private var toolbarTitle: String = ""
    @State private var showTabsDemo: Bool = false

    var body: some View {
        ZStack(alignment: .topLeading) {
            #if os(iOS)
            // Mobile: hide the sidebar for now; show only the latest thread
            AcpThreadView(url: nil, initialLines: awaitLatestLines(), onTitleChange: { t in
                self.toolbarTitle = t
            })
            .navigationTitle("")
            #else
            NavigationSplitView {
                HistorySidebar(selected: selectedRow, onSelect: { row, url in
                    withAnimation(.easeInOut(duration: 0.15)) {
                        self.selectedRow = row
                        self.selectedURL = url
                    }
                })
                .navigationSplitViewColumnWidth(min: 220, ideal: 260)
            } detail: {
                // Keep detail title empty; we draw our own leading toolbar title
                AcpThreadView(url: selectedURL, onTitleChange: { t in
                    self.toolbarTitle = t
                })
                .navigationTitle("")
            }
            #endif
            // Gradient sits above content but under the toolbar, creating a soft edge behind the title
            TopEdgeGradient()
        }
        .background(OATheme.Colors.background.ignoresSafeArea())
        .task { FMProbe.logAvailability() }
        // Floating toolbar: iPhone only; overlays bottom-right above content
        #if os(iOS)
        .overlay(alignment: .bottomTrailing) {
            FloatingToolbar()
        }
        // .overlay(alignment: .bottomTrailing) {
        //     FloatingMicButton()
        // }
        // .overlay(alignment: .topTrailing) {
        //     FloatingMenuButton(onTap: { showTabsDemo = true })
        // }
        #endif
        // Present Chat Tabs demo from menu button (temporarily disabled)
        // #if os(iOS)
        // .sheet(isPresented: $showTabsDemo) {
        //     if #available(iOS 26, *) {
        //         ChatTabsDemo()
        //             .preferredColorScheme(.dark)
        //     } else {
        //         VStack(spacing: 12) {
        //             Image(systemName: "info.circle")
        //                 .font(.system(size: 28, weight: .regular))
        //             Text("Tabs demo requires iOS 26+")
        //                 .font(.headline)
        //             Button("Close") { showTabsDemo = false }
        //                 .padding(.top, 8)
        //         }
        //         .padding(24)
        //     }
        // }
        // #endif
        #if os(iOS)
        .toolbar(.visible, for: .navigationBar)
        .toolbarBackground(.hidden, for: .navigationBar)
        .toolbarColorScheme(.dark, for: .navigationBar)
        .toolbar {
            ToolbarItem(placement: .navigationBarLeading) {
                Text(selectedRowTitle())
                    .font(OAFonts.ui(.headline, 15))
                    .foregroundStyle(OATheme.Colors.textPrimary)
                    .lineLimit(1)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 6)
            }
        }
        #endif
        #if os(macOS)
        .toolbar(.visible, for: .windowToolbar)
        .toolbarBackground(.hidden, for: .windowToolbar)
        .toolbar {
            ToolbarItem(placement: .navigation) {
                Text(selectedRowTitle())
                    .font(OAFonts.ui(.headline, 14))
                    .foregroundStyle(OATheme.Colors.textPrimary)
                    .lineLimit(1)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 6)
            }
        }
        #endif
        .preferredColorScheme(.dark)
    }

    #if os(iOS)
    @EnvironmentObject private var bridge: BridgeManager
    private func awaitLatestLines() -> [String] {
        // BridgeManager publishes latestLines; read current snapshot for initial render
        return bridge.latestLines
    }
    #endif

    private func selectedRowTitle() -> String {
        if let r = selectedRow {
            if !toolbarTitle.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { return sanitizeTitle(toolbarTitle) }
            if let t = r.title, !t.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { return sanitizeTitle(t) }
            return "Thread"
        }
        return "OpenAgents"
    }

    private func sanitizeTitle(_ s: String) -> String {
        var t = s
        if let rx = try? NSRegularExpression(pattern: "\\[([^\\]]+)\\]\\([^\\)]+\\)", options: []) {
            t = rx.stringByReplacingMatches(in: t, range: NSRange(location: 0, length: t.utf16.count), withTemplate: "$1")
        }
        for mark in ["**","*","__","_","```,","`"] { t = t.replacingOccurrences(of: mark, with: "") }
        t = t.replacingOccurrences(of: "\\s+", with: " ", options: .regularExpression)
        return t.trimmingCharacters(in: .whitespacesAndNewlines)
    }
}

#Preview {
    ContentView()
}
