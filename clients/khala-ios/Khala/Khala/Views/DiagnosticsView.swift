import SwiftUI

struct DiagnosticsView: View {
    @Environment(\.dismiss) private var dismiss

    let snapshot: LocalDiagnosticsSnapshot
    @State private var showOwnerRawDetail = false

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    ForEach(snapshot.publicRows) { row in
                        DiagnosticsRow(row: row)
                    }
                } header: {
                    Text("Public-safe summary")
                } footer: {
                    Text("This default view excludes prompts, raw command output, local paths, credentials, wallet material, and private repository data.")
                }

                Section {
                    Toggle(isOn: $showOwnerRawDetail) {
                        Label("Show owner-only raw local detail", systemImage: "lock.open")
                    }
                    .tint(.orange)

                    if showOwnerRawDetail {
                        ForEach(snapshot.rawRows) { row in
                            DiagnosticsRow(row: row)
                        }
                    }
                } header: {
                    Text("Owner-only local access")
                } footer: {
                    Text("Raw diagnostics stay on this device. Values are still redacted before rendering and must not be copied into public reports or projections.")
                }
            }
            .navigationTitle("Diagnostics")
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { dismiss() }
                }
            }
        }
    }
}

private struct DiagnosticsRow: View {
    let row: LocalDiagnosticsSnapshot.Row

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            Image(systemName: row.systemImage)
                .frame(width: 22)
                .foregroundStyle(.secondary)
            VStack(alignment: .leading, spacing: 3) {
                Text(row.label)
                    .font(.subheadline.weight(.semibold))
                Text(row.value)
                    .font(.footnote)
                    .foregroundStyle(.secondary)
                    .textSelection(.enabled)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .padding(.vertical, 2)
        .accessibilityElement(children: .combine)
    }
}
