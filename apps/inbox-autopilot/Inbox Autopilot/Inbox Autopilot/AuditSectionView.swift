import SwiftUI

struct AuditSectionView: View {
    @EnvironmentObject private var model: AppModel

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Audit Timeline")
                    .font(.title3)
                    .fontWeight(.semibold)
                Spacer()

                Button("Refresh") {
                    Task {
                        await model.refreshEvents()
                        if let threadID = model.selectedThreadID {
                            await model.openThread(id: threadID)
                        }
                    }
                }

                Button("Export Selected Thread") {
                    Task { await model.exportSelectedAudit() }
                }
                .disabled(model.selectedThreadID == nil)

                Button("Evaluate Draft Quality") {
                    Task { await model.refreshDraftQualityReport() }
                }
                .disabled(!model.daemonConnected)
            }

            if let report = model.draftQualityReport {
                qualityReportView(report)
            }

            if let audit = model.threadAudit {
                HStack(spacing: 8) {
                    if let category = audit.category {
                        pill(category.title, color: .blue)
                    }
                    if let risk = audit.risk {
                        pill(risk.title, color: risk == .high ? .red : (risk == .medium ? .orange : .green))
                    }
                    if let policy = audit.policy {
                        pill(policy.title, color: .purple)
                    }
                    if audit.externalModelUsed {
                        pill("External model", color: .mint)
                    } else {
                        pill("Local model", color: .gray)
                    }
                }

                if !audit.similarThreadIDs.isEmpty {
                    Text("Similar threads: \(audit.similarThreadIDs.joined(separator: ", "))")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }

            List(model.events) { event in
                VStack(alignment: .leading, spacing: 4) {
                    HStack {
                        Text(event.eventType)
                            .font(.headline)
                        Spacer()
                        Text(event.createdAt.formatted(date: .abbreviated, time: .standard))
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }

                    if let threadID = event.threadID {
                        Text("Thread: \(threadID)")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }

                    if !event.payload.isEmpty {
                        Text(event.payload.map { "\($0.key)=\($0.value.value)" }.sorted().joined(separator: "  •  "))
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .lineLimit(3)
                    }
                }
                .padding(.vertical, 4)
            }
        }
        .padding(14)
    }

    private func qualityReportView(_ report: DraftQualityReport) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text("Draft Quality")
                    .font(.headline)
                Spacer()
                Text(report.generatedAt.formatted(date: .abbreviated, time: .shortened))
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            HStack(spacing: 8) {
                pill(
                    report.targetMet ? "Target met" : "Target not met",
                    color: report.targetMet ? .green : .orange
                )
                pill(
                    "Minimal edit \(percentString(report.totalMinimalEditRate))",
                    color: .mint
                )
                pill("Samples \(report.totalSamples)", color: .gray)
            }

            ForEach(report.categories) { category in
                HStack {
                    Text(category.category.title)
                    Spacer()
                    Text("\(category.minimalEditRate, format: .percent.precision(.fractionLength(0)))")
                        .fontWeight(.semibold)
                    Text("(\(category.minimalEditCount)/\(category.samples))")
                        .foregroundStyle(.secondary)
                }
                .font(.caption)
            }
        }
        .padding(10)
        .background(Color.gray.opacity(0.1), in: RoundedRectangle(cornerRadius: 10))
    }

    private func pill(_ text: String, color: Color) -> some View {
        Text(text)
            .font(.caption)
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(color.opacity(0.18), in: Capsule())
    }

    private func percentString(_ value: Double) -> String {
        String(format: "%.0f%%", value * 100)
    }
}
