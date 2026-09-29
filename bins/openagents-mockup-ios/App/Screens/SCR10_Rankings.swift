import SwiftUI

// SCR-10 Rankings (later). The collective, and a reason to come back.

enum SCR10State: String, Hashable, CaseIterable {
    case week, season, empty, offline
}

struct SCR10Rankings: View {
    @Environment(MockApp.self) private var app
    let state: SCR10State
    @State private var season = false

    var body: some View {
        if state == .offline {
            PAT01States(state: .offlineMenu)
        } else {
            ScreenScaffold {
                TopBar(back: BackControl(label: "Menu") { app.backToMenu() }, title: "Rankings")
            } content: {
                // E01
                Picker("", selection: $season) {
                    Text("This week").tag(false)
                    Text("Season").tag(true)
                }
                .pickerStyle(.segmented)

                if state == .empty && !season {
                    Text("The week just started. Train first to top the list.")
                        .font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                        .padding(.vertical, Theme.Space.xl)
                } else {
                    // E02
                    VStack(spacing: 0) {
                        ForEach(season ? MockData.rankingsSeason : MockData.rankingsWeek) { r in
                            if r.place > 6 && !season {
                                Text("…").foregroundStyle(Theme.Colors.textTertiary).padding(.vertical, 4)
                            }
                            rankRow(place: r.place, name: r.name, level: r.level, xp: r.xp, you: false)
                            Divider().overlay(Theme.Colors.divider)
                        }
                    }
                }
            } bottom: {
                // E03 (pinned)
                VStack(alignment: .leading, spacing: 2) {
                    rankRow(place: MockData.yourRank, name: "YOU · \(MockData.player.name)", level: app.level,
                            xp: app.xp + MockData.xpPerRun, you: true)
                    Text("\(MockData.xpToPassNext) XP to pass #\(MockData.yourRank - 1)")
                        .font(Theme.Fonts.caption).foregroundStyle(Theme.Colors.textSecondary)
                        .padding(.leading, 44)
                }
                .padding(.horizontal, Theme.Space.s)
                .padding(.vertical, Theme.Space.xs)
                .background(RoundedRectangle(cornerRadius: Theme.Radius.card).fill(Theme.Colors.surfaceRaised))
                // E04, E05
                NextLine(text: "one more run could pass #\(MockData.yourRank - 1).")
                PrimaryButton(title: "Climb: enter the gym") { app.go(.gym(.returning)) }
            }
            .onAppear { season = state == .season }
        }
    }

    private func rankRow(place: Int, name: String, level: Int, xp: Int, you: Bool) -> some View {
        HStack(spacing: Theme.Space.s) {
            Text("\(place)").font(Theme.Fonts.bodyBold).frame(width: 32, alignment: .trailing)
            Text(name).font(you ? Theme.Fonts.bodyBold : Theme.Fonts.body).lineLimit(1)
            Spacer()
            Text("Level \(level)").font(Theme.Fonts.caption).foregroundStyle(Theme.Colors.textSecondary)
            Text("\(xp.formatted()) XP").font(Theme.Fonts.bodyBold).frame(minWidth: 80, alignment: .trailing)
        }
        .padding(.vertical, 12)
    }
}

#Preview("SCR-10 Rankings") {
    NavigationStack { SCR10Rankings(state: .week) }.environment(MockApp())
}

#Preview("SCR-10 Empty week") {
    NavigationStack { SCR10Rankings(state: .empty) }.environment(MockApp())
}
