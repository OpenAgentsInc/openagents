import SwiftUI

// SCR-18 Chat: Wrong answer. Inline under a prepared answer, not a new
// screen. Says exactly what leaves the phone before anything does.

enum SCR18State: String, Hashable, CaseIterable {
    case idle, confirming, sending, sent
}

struct SCR18WrongAnswer: View {
    var start: SCR18State = .idle
    @State private var phase: SCR18State = .idle

    var body: some View {
        VStack(alignment: .leading, spacing: Theme.Space.s) {
            switch phase {
            case .idle:
                // E01
                Chip(icon: "flag", text: "Wrong answer") { withAnimation { phase = .confirming } }
            case .confirming:
                // E02
                Text("We'll send your question, our answer, and how we chose it to the OpenAgents team, encrypted, to improve our answers. Nothing else from this chat is sent.")
                    .font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                    .fixedSize(horizontal: false, vertical: true)
                // E03, E04
                HStack(spacing: Theme.Space.s) {
                    Chip(text: "Send", filled: true) { send() }
                    Chip(text: "Cancel") { withAnimation { phase = .idle } }
                }
            case .sending:
                // E05
                HStack(spacing: 8) {
                    ProgressView().tint(Theme.Colors.textSecondary)
                    Text("Sending…").font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                }
            case .sent:
                Label("Sent to the OpenAgents team as \(MockData.reportCode). Thank you.", systemImage: "checkmark")
                    .font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
            }
        }
        .padding(phase == .confirming ? Theme.Space.s : 0)
        .background {
            if phase == .confirming {
                RoundedRectangle(cornerRadius: 12).stroke(Theme.Colors.stroke, lineWidth: 1)
            }
        }
        .onAppear { phase = start }
    }

    private func send() {
        withAnimation { phase = .sending }
        Task {
            try? await Task.sleep(for: .seconds(0.9))
            withAnimation { phase = .sent }
        }
    }
}

#Preview("SCR-18 Wrong answer") {
    VStack(alignment: .leading, spacing: 30) {
        SCR18WrongAnswer(start: .idle)
        SCR18WrongAnswer(start: .confirming)
        SCR18WrongAnswer(start: .sent)
    }
    .padding()
    .frame(maxHeight: .infinity)
    .background(Theme.Colors.background)
}
