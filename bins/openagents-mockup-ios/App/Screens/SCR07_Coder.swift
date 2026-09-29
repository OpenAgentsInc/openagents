import SwiftUI

// SCR-07 Coder: your agent (later). Score today, a chart, and the tools
// Coder uses now.

enum SCR07State: String, Hashable, CaseIterable {
    case normal, loading, emptyTesting, error
}

struct SCR07Coder: View {
    @Environment(MockApp.self) private var app
    let state: SCR07State

    var body: some View {
        if state == .error {
            PAT01States(state: .ourError)
        } else {
            ScreenScaffold {
                TopBar(back: BackControl(label: "Menu") { app.backToMenu() }, title: "Coder")
            } content: {
                // E01, E02
                HStack(spacing: Theme.Space.m) {
                    CoderFigure().frame(height: 130)
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Score today").font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                        if state == .loading {
                            LoadingBar(width: 120, height: 36)
                        } else {
                            Text("\(MockData.coderScoreAfter) of \(MockData.practiceTasks)").font(Theme.Fonts.hugeNumber)
                        }
                        Text("practice tasks").font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                    }
                }
                // E03
                Card {
                    VStack(alignment: .leading, spacing: 8) {
                        SectionLabel(text: "Score by week")
                        if state == .loading { LoadingBar(width: 260, height: 90) } else { chart }
                    }
                }
                // E04
                SectionLabel(text: "Tools Coder uses now")
                ListRow(icon: "map", title: "Project map",
                        subtitle: "Confirmed by \(MockData.tools[0].confirmedBy) trainers · added Oct 3")
                // E05
                SectionLabel(text: "Being tested")
                if state == .emptyTesting {
                    Text("Nothing is being tested. Be the first.").font(Theme.Fonts.body)
                        .foregroundStyle(Theme.Colors.textSecondary)
                } else {
                    ListRow(icon: "magnifyingglass", title: "Code finder", subtitle: "2 checks so far") {
                        app.go(.toolDetail("code-finder"))
                    }
                }
            } bottom: {
                // E06, E07
                NextLine(text: "train Coder to raise its score.")
                PrimaryButton(title: "Train Coder") { app.go(.gym(.returning)) }
            }
        }
    }

    private var chart: some View {
        HStack(alignment: .bottom, spacing: Theme.Space.l) {
            ForEach(Array(MockData.coderScoreHistory.enumerated()), id: \.offset) { i, score in
                VStack(spacing: 6) {
                    Text("\(score)").font(Theme.Fonts.bodyBold)
                    RoundedRectangle(cornerRadius: 4)
                        .fill(i == MockData.coderScoreHistory.count - 1 ? Theme.Colors.textPrimary : Theme.Colors.surfaceRaised)
                        .overlay(RoundedRectangle(cornerRadius: 4).stroke(Theme.Colors.stroke, lineWidth: 1))
                        .frame(width: 44, height: CGFloat(score) * 11)
                    Text(MockData.coderScoreWeeks[i]).font(Theme.Fonts.caption).foregroundStyle(Theme.Colors.textSecondary)
                }
            }
        }
        .frame(maxWidth: .infinity)
    }
}

#Preview("SCR-07 Coder") {
    NavigationStack { SCR07Coder(state: .normal) }.environment(MockApp())
}

#Preview("SCR-07 Loading") {
    NavigationStack { SCR07Coder(state: .loading) }.environment(MockApp())
}
