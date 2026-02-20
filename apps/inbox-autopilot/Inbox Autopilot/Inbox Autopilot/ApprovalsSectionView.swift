import SwiftUI

struct ApprovalsSectionView: View {
    @EnvironmentObject private var model: AppModel
    let onOpenThread: (String) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Approvals Queue")
                    .font(.title3)
                    .fontWeight(.semibold)
                Spacer()
                Button("Refresh") {
                    Task { await model.refreshPendingDrafts() }
                }
            }

            if model.pendingDrafts.isEmpty {
                Spacer()
                Text("No drafts pending approval.")
                    .foregroundStyle(.secondary)
                Spacer()
            } else {
                List(model.pendingDrafts) { draft in
                    VStack(alignment: .leading, spacing: 8) {
                        HStack {
                            Text("Thread: \(draft.threadID)")
                                .font(.headline)
                            Spacer()
                            Text(draft.status.title)
                                .font(.caption)
                                .padding(.horizontal, 6)
                                .padding(.vertical, 2)
                                .background(Color.mint.opacity(0.2), in: Capsule())
                        }

                        Text(draft.body)
                            .lineLimit(4)
                            .font(.body)

                        HStack(spacing: 8) {
                            Button("Open Thread") {
                                onOpenThread(draft.threadID)
                            }
                            Button("Mark Needs Human") {
                                Task { await model.markNeedsHuman(draftID: draft.id) }
                            }
                        }
                    }
                    .padding(.vertical, 6)
                }
            }
        }
        .padding(14)
    }
}
