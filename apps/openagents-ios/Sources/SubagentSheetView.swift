// Firm reason: 6 (iOS SwiftUI sheet wrapper around shared SubagentInspectorView for reading live subagent transcripts and stopping subagents).
import SwiftUI

struct SubagentSheetView: View {
    @ObservedObject var store: AppStore
    let subagent: TranscriptItem
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            SubagentInspectorView(store: store, subagent: subagent)
                .navigationTitle("Subagent")
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .topBarTrailing) {
                        Button("Done") {
                            store.closeSubagent()
                            dismiss()
                        }
                        .accessibilityIdentifier("subagent.done")
                    }
                }
        }
        .preferredColorScheme(.dark)
    }
}
