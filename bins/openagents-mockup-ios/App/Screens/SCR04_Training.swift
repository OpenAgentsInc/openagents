import SwiftUI

// SCR-04 Training. First run step 3 of 3. A fake run lights one block per
// task over MockData.fakeTrainingSeconds.

enum SCR04State: String, Hashable, CaseIterable {
    case firstRun, running, check, done, slow, failed, offline
}

struct SCR04Training: View {
    @Environment(MockApp.self) private var app
    let state: SCR04State
    @State private var tasksDone = 0
    @State private var solved = 0
    @State private var started = false

    private var total: Int { MockData.practiceTasks }
    private var finished: Bool { tasksDone >= total }
    private var minutesLeft: Int { max(1, Int((Double(total - tasksDone) / Double(total) * 5).rounded(.up))) }

    var body: some View {
        if state == .failed {
            PAT01States(state: .trainingFailed)
        } else {
            ScreenScaffold {
                // E01
                TopBar(back: state == .firstRun ? nil : BackControl(label: "Menu") { app.backToMenu() },
                       title: state == .check ? "Checking" : "Training",
                       step: state == .firstRun ? 3 : nil)
            } content: {
                // E02
                Text(state == .check
                     ? "Coder is rerunning \(MockData.checkTrainer)'s result with \(MockData.checkTool)."
                     : "Coder is practicing with \(app.selectedTool.name).")
                    .font(Theme.Fonts.title)
                    .fixedSize(horizontal: false, vertical: true)

                if state == .offline {
                    StatusPill(text: "You're offline. Training continues.", live: false)
                }

                // E03
                ZStack {
                    LinearGradient(colors: [Color(white: 0.1), .black], startPoint: .top, endPoint: .bottom)
                    GridFloor(horizon: 0.5, lines: 18)
                    HStack(alignment: .bottom, spacing: 20) {
                        CoderFigure(glow: finished ? 1 : 0.6).frame(height: 150)
                        workbench
                    }
                    .padding(.bottom, 10)
                }
                .frame(height: 200)
                .clipShape(RoundedRectangle(cornerRadius: Theme.Radius.card))
                .overlay(RoundedRectangle(cornerRadius: Theme.Radius.card).stroke(Theme.Colors.stroke, lineWidth: 1))

                ProgressBlocks(done: tasksDone, total: total)

                // E04, E05
                VStack(alignment: .leading, spacing: 6) {
                    Text(finished ? "All \(total) practice tasks done" : "Practice task \(min(tasksDone + 1, total)) of \(total)")
                        .font(Theme.Fonts.bodyBold)
                    Text("Solved so far: \(solved)").font(Theme.Fonts.body)
                    Text("Coder's score before: \(MockData.coderScoreBefore) of \(total)")
                        .font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                    if !finished {
                        Text(state == .slow ? "Working" : "About \(minutesLeft) minutes left")
                            .font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                    }
                }

                // E06
                Text(reassurance)
                    .font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                    .fixedSize(horizontal: false, vertical: true)
            } bottom: {
                // E07, E08, E09
                NextLine(text: finished ? "see how Coder did." : "wait for the result, or come back.")
                PrimaryButton(title: "See the result",
                              detail: finished ? nil : "Ready in about \(minutesLeft) minutes",
                              enabled: finished) {
                    app.go(.result(state == .check ? .checkConfirmed : .better))
                }
                SecondaryLink(title: "Ask OpenAgents while you wait") { app.go(.newChat(.fromTraining)) }
            }
            .task { await run() }
        }
    }

    private var reassurance: String {
        switch state {
        case .slow: "Taking longer than usual. We'll keep going."
        case .offline: "You're offline. Training continues. We'll show the result when you're back."
        default: finished ? "Done. Your result is ready." : "You can leave. We'll keep going and show you the result here."
        }
    }

    private var workbench: some View {
        VStack(spacing: 4) {
            ForEach(0..<3, id: \.self) { row in
                HStack(spacing: 4) {
                    ForEach(0..<4, id: \.self) { col in
                        let i = row * 4 + col
                        RoundedRectangle(cornerRadius: 2)
                            .fill(i < tasksDone ? Color.white : Color.white.opacity(0.08))
                            .frame(width: 16, height: 10)
                            .opacity(i < total ? 1 : 0)
                    }
                }
            }
            Rectangle().fill(.white.opacity(0.7)).frame(width: 90, height: 3)
        }
    }

    /// The fake run.
    private func run() async {
        guard !started else { return }
        started = true
        switch state {
        case .done:
            tasksDone = total; solved = MockData.coderScoreAfter; app.trainingFinished = true; return
        case .slow:
            tasksDone = 7; solved = 5; return
        default: break
        }
        tasksDone = state == .offline ? 4 : 0
        solved = state == .offline ? 3 : 0
        let step = MockData.fakeTrainingSeconds / Double(total)
        while tasksDone < total {
            try? await Task.sleep(for: .seconds(step))
            if Task.isCancelled { return }
            tasksDone += 1
            // Solve 8 of 10, missing tasks 3 and 7.
            if tasksDone != 3 && tasksDone != 7 { solved += 1 }
        }
        app.trainingFinished = true
        UINotificationFeedbackGenerator().notificationOccurred(.success)
    }
}

#Preview("SCR-04 Training") {
    NavigationStack { SCR04Training(state: .firstRun) }.environment(MockApp())
}

#Preview("SCR-04 Done") {
    NavigationStack { SCR04Training(state: .done) }.environment(MockApp())
}

#Preview("SCR-04 Slow") {
    NavigationStack { SCR04Training(state: .slow) }.environment(MockApp())
}
