import SwiftUI

struct NodeInspectorView: View {
    let hasKey: Bool
    let isUsingEphemeralFallback: Bool

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                statusCard(
                    title: "Khala Chat",
                    rows: [
                        StatusRow(label: "Model", value: "openagents/khala", systemImage: "sparkles", tone: .ready),
                        StatusRow(label: "API key", value: hasKey ? "stored in Keychain" : "missing", systemImage: "key", tone: hasKey ? .ready : .blocked),
                        StatusRow(label: "History", value: isUsingEphemeralFallback ? "memory only" : "local file", systemImage: "tray.full", tone: isUsingEphemeralFallback ? .blocked : .ready),
                    ]
                )

                statusCard(
                    title: "Local Pylon",
                    rows: [
                        StatusRow(label: "Supervisor", value: "not connected", systemImage: "bolt.horizontal.circle", tone: .neutral),
                        StatusRow(label: "Heartbeat", value: "offline", systemImage: "waveform.path.ecg", tone: .neutral),
                        StatusRow(label: "Provider mode", value: "offline", systemImage: "power.circle", tone: .neutral),
                    ]
                )

                statusCard(
                    title: "Apple FM",
                    rows: [
                        StatusRow(label: "Bridge", value: "unavailable", systemImage: "cpu", tone: .neutral),
                        StatusRow(label: "Backend", value: "apple_fm_bridge", systemImage: "shippingbox", tone: .neutral),
                        StatusRow(label: "Usage truth", value: "not measured", systemImage: "number", tone: .neutral),
                    ]
                )

                statusCard(
                    title: "Fleet",
                    rows: [
                        StatusRow(label: "Connected accounts", value: "none loaded", systemImage: "person.2", tone: .neutral),
                        StatusRow(label: "Assignments", value: "none active", systemImage: "checklist", tone: .neutral),
                        StatusRow(label: "Receipts", value: "not connected", systemImage: "doc.text.magnifyingglass", tone: .neutral),
                    ]
                )
            }
            .padding(20)
        }
        .navigationSplitViewColumnWidth(min: 260, ideal: 320, max: 420)
        .background(Color(nsColor: .windowBackgroundColor))
    }

    private func statusCard(title: String, rows: [StatusRow]) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(title)
                .font(.headline)
            ForEach(rows) { row in
                HStack(alignment: .firstTextBaseline, spacing: 10) {
                    Image(systemName: row.systemImage)
                        .foregroundStyle(row.tone.color)
                        .frame(width: 18)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(row.label)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        Text(row.value)
                            .font(.callout.weight(.medium))
                    }
                    Spacer(minLength: 0)
                }
            }
        }
        .padding(14)
        .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 8))
    }
}

private struct StatusRow: Identifiable {
    let id = UUID()
    let label: String
    let value: String
    let systemImage: String
    let tone: Tone

    enum Tone {
        case ready
        case neutral
        case blocked

        var color: Color {
            switch self {
            case .ready: return .green
            case .neutral: return .secondary
            case .blocked: return .orange
            }
        }
    }
}
