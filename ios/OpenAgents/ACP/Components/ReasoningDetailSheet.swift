import SwiftUI
import OpenAgentsCore

/// Modular detail sheet for displaying full reasoning/thinking content
struct ReasoningDetailSheet: View {
    let summary: ReasoningSummary
    @Binding var isPresented: Bool

    var body: some View {
        NavigationStack {
            List {
                ForEach(Array(summary.thoughts.enumerated()), id: \.offset) { index, thought in
                    Text(thought)
                        .font(OAFonts.ui(.body, 14))
                        .foregroundStyle(OATheme.Colors.textPrimary)
                        .textSelection(.enabled)
                }
            }
            #if os(iOS)
            .listStyle(.insetGrouped)
            #else
            .listStyle(.inset)
            #endif
            .navigationTitle("Thoughts")
            .toolbar {
                #if os(iOS)
                ToolbarItem(placement: .topBarLeading) {
                    Button("Close") { isPresented = false }
                }
                #else
                ToolbarItem(placement: .navigation) {
                    Button("Close") { isPresented = false }
                }
                #endif
            }
        }
    }
}
