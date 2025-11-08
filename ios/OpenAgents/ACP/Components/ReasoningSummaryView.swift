import SwiftUI
import OpenAgentsCore

/// Modular component for displaying consolidated agent reasoning/thinking
struct ReasoningSummaryView: View {
    let summary: ReasoningSummary
    let onTap: () -> Void

    var body: some View {
        let duration = formatDuration(seconds: summary.durationSeconds)

        Button(action: onTap) {
            Text("Thought for \(duration)")
                .font(.footnote)
                .foregroundStyle(OATheme.Colors.textSecondary)
                .padding(.horizontal, 10)
                .padding(.vertical, 6)
                .background(
                    Group {
                        #if os(iOS)
                        if #available(iOS 26, *) {
                            GlassEffectContainer {
                                Capsule(style: .continuous)
                                    .fill(Color.clear)
                                    .glassEffect(.regular, in: Capsule(style: .continuous))
                            }
                        } else {
                            Capsule(style: .continuous).fill(.ultraThinMaterial)
                        }
                        #else
                        Capsule(style: .continuous).fill(.regularMaterial)
                        #endif
                    }
                )
                .background(
                    Capsule(style: .continuous)
                        .fill(LinearGradient(
                            colors: [Color.black.opacity(0.14), Color.black.opacity(0.05)],
                            startPoint: .top,
                            endPoint: .bottom
                        ))
                )
                .overlay(
                    Capsule(style: .continuous)
                        .strokeBorder(OATheme.Colors.border.opacity(0.6), lineWidth: 1)
                )
                .clipShape(Capsule(style: .continuous))
        }
        .buttonStyle(.plain)
    }

    private func formatDuration(seconds: Int) -> String {
        if seconds < 60 {
            return "\(seconds)s"
        } else if seconds < 3600 {
            let mins = seconds / 60
            let secs = seconds % 60
            return secs > 0 ? "\(mins)m \(secs)s" : "\(mins)m"
        } else {
            let hrs = seconds / 3600
            let mins = (seconds % 3600) / 60
            return mins > 0 ? "\(hrs)h \(mins)m" : "\(hrs)h"
        }
    }
}

/// Data model for reasoning summary
struct ReasoningSummary: Identifiable, Equatable {
    let id: String
    let startTs: Int64
    let endTs: Int64
    let thoughts: [String]

    var durationSeconds: Int {
        max(0, Int((endTs - startTs) / 1000))
    }

    init(id: String = UUID().uuidString, startTs: Int64, endTs: Int64, thoughts: [String]) {
        self.id = id
        self.startTs = startTs
        self.endTs = endTs
        self.thoughts = thoughts
    }
}
