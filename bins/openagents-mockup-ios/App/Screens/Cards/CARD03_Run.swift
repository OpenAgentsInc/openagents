import SwiftUI

// CARD-03 Run card: a run in progress. It replaces the card that started
// it, and a CARD-04 replaces it when it finishes. Replaces the retired
// SCR-04 page. The fake run takes MockData.fakeRunSeconds (12 s).

enum CARD03State: String, Hashable, CaseIterable {
    case running, slow, failed, offline
}

struct CARD03Run: View {
    let kind: RunKind
    var state: CARD03State = .running
    var startedAt = Date()
    /// Step 3 of 3 in the first-run chat.
    var firstRun = false
    var onDone: () -> Void = {}
    var onStop: () -> Void = {}
    var onRetry: () -> Void = {}

    @State private var progress: Double = 0
    @State private var confirmStop = false

    private var total: Int { kind.total }
    /// Tests finished on each side; "with the tool" runs a little ahead.
    private var doneWith: Int { min(total, Int((progress * 1.1) * Double(total))) }
    private var doneWithout: Int { min(total, Int(progress * Double(total))) }
    private var minutesLeft: Int { max(1, Int(((1 - progress) * Double(kind.minutes)).rounded(.up))) }

    var body: some View {
        if state == .failed {
            // PAT-01 inside the card.
            PAT01Inline(icon: "exclamationmark.triangle", what: "Something went wrong on our side.",
                        means: "This didn't use a run.", reassure: nil, code: "RUN-03", onRetry: onRetry)
        } else {
            ChatCardFrame(highlighted: true) {
                // E01
                HStack {
                    Text(kind.title).condensedTitle(Theme.Fonts.cardTitle)
                        .fixedSize(horizontal: false, vertical: true)
                    Spacer()
                    if firstRun { StepDots(step: 3) }
                }
                if firstRun {
                    Text("STEP 3 OF 3").condensedTitle(Theme.Fonts.sectionLabel, tracking: Theme.Tracking.sectionLabel)
                        .foregroundStyle(Theme.Colors.textTertiary)
                }
                // E02
                blocks(done: doneWith, label: "with the tool")
                blocks(done: doneWithout, label: "without it")
                // E03
                Text(counter).font(Theme.Fonts.bodyBold)
                // E04
                CardNote(text: reassurance)
                // E05
                Chip(icon: "stop.fill", text: "Stop") { confirmStop = true }
                    .confirmationDialog("Stop the test? It won't use a run.", isPresented: $confirmStop,
                                        titleVisibility: .visible) {
                        Button("Stop the test", role: .destructive) { onStop() }
                        Button("Keep going", role: .cancel) {}
                    }
            }
            .task(id: startedAt) { await run() }
        }
    }

    private var counter: String {
        let n = min(doneWithout + 1, total)
        switch state {
        case .slow: return "Test \(n) of \(total) · Working"
        case .offline: return "Test \(n) of \(total) · Last seen 10:42"
        default: return "Test \(n) of \(total) · about \(minutesLeft) minutes left"
        }
    }

    private var reassurance: String {
        switch state {
        case .slow: "Taking longer than usual. We'll keep going."
        case .offline: "You're offline. The test keeps going on our computers; this card updates when you're back."
        default: "You can leave. We'll post the result here and on the menu."
        }
    }

    private func blocks(done: Int, label: String) -> some View {
        HStack(spacing: Theme.Space.s) {
            HStack(spacing: 4) {
                ForEach(0..<total, id: \.self) { i in
                    RoundedRectangle(cornerRadius: 3)
                        .fill(i < done ? Theme.Colors.textPrimary : Theme.Colors.surfaceRaised)
                        .overlay(RoundedRectangle(cornerRadius: 3).stroke(Theme.Colors.stroke, lineWidth: 1))
                        .frame(width: Theme.Size.runBlock, height: Theme.Size.runBlock)
                        .themeShadow(Theme.Shadow(color: i < done ? .white.opacity(0.45) : .clear, radius: 4, y: 0))
                }
            }
            .animation(Theme.Motion.reveal, value: done)
            Text(label).font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                .lineLimit(1).minimumScaleFactor(0.8)
        }
    }

    /// The fake run: progress from the start time, so leaving and coming
    /// back (or relaunching in FLOW-01) picks up where it was.
    private func run() async {
        switch state {
        case .slow: progress = 0.84; return
        case .offline: progress = 0.45; return
        case .failed: return
        case .running: break
        }
        while !Task.isCancelled {
            let p = Date().timeIntervalSince(startedAt) / kind.seconds
            progress = min(p, 1)
            if p >= 1 {
                UINotificationFeedbackGenerator().notificationOccurred(.success)
                onDone()
                return
            }
            try? await Task.sleep(for: .seconds(0.2))
        }
    }
}

#Preview("CARD-03 Running") {
    ScrollView { CARD03Run(kind: .tool("project-map")).padding() }.background(Theme.Colors.background)
}

#Preview("CARD-03 First run") {
    ScrollView { CARD03Run(kind: .tool("project-map"), firstRun: true).padding() }.background(Theme.Colors.background)
}

#Preview("CARD-03 Slow") {
    ScrollView { CARD03Run(kind: .tool("project-map"), state: .slow).padding() }.background(Theme.Colors.background)
}

#Preview("CARD-03 Checking") {
    ScrollView { CARD03Run(kind: .check).padding() }.background(Theme.Colors.background)
}

#Preview("CARD-03 Failed") {
    ScrollView { CARD03Run(kind: .tool("project-map"), state: .failed).padding() }.background(Theme.Colors.background)
}
