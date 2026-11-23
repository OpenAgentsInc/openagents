// Firm reason: 6 (iOS SwiftUI sheet wrapper around shared WorkspaceInspectorView for reviewing Git diffs, branches, worktrees, and PRs).
import SwiftUI

struct WorkspaceView: View {
    @ObservedObject var store: AppStore
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            WorkspaceInspectorView(store: store)
                .navigationTitle("Workspace")
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .topBarTrailing) {
                        Button("Done") { dismiss() }
                            .accessibilityIdentifier("workspace.done")
                    }
                }
        }
        .preferredColorScheme(.dark)
    }
}
