import SwiftUI

// SCR-01 Main menu. The hub the player returns to.

enum SCR01State: String, Hashable, CaseIterable {
    case normal, loading, checkWaiting, noRunsLeft, trainingInProgress, offline, v1Cut
}

struct SCR01MainMenu: View {
    @Environment(MockApp.self) private var app
    let state: SCR01State

    private var showLater: Bool { MockData.showLaterFeatures && state != .v1Cut }

    private var nextLine: String {
        switch state {
        case .checkWaiting: "check another trainer's result (+\(MockData.xpPerRun) XP)."
        case .noRunsLeft: "new runs at 9:00 tomorrow. You can still check results now."
        case .offline: "you're offline. We'll update when you're back."
        default: "give Coder a new tool."
        }
    }

    private let gymTitle = "Enter the gym"
    private var gymSubtitle: String {
        switch state {
        case .checkWaiting: "A check is waiting for you"
        case .noRunsLeft: "Check results while you wait"
        default: "Make Coder better · \(app.runsLeft) runs left today"
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            // E01, E02
            HStack {
                LogoWordmark()
                Spacer()
                if showLater { BellButton(count: 2) { app.go(.updates(.normal)) } }
            }
            .padding(.horizontal, Theme.Space.page)
            .frame(height: Theme.Size.topBarHeight)

            ScrollView {
                VStack(spacing: Theme.Space.s) {
                    // E04
                    PlayerCard(level: app.level, xp: app.xp, xpForNext: app.xpForNextLevel,
                               loading: state == .loading) { app.go(.profile(.normal)) }

                    // E05
                    HeroArt()
                        .frame(height: Theme.Size.heroHeight)
                        .overlay(alignment: .bottom) {
                            Group {
                                if state == .loading {
                                    StatusPill(text: "GYM OPEN · Not known yet")
                                } else if state == .offline {
                                    StatusPill(text: "OFFLINE · Last updated 10:42", live: false)
                                } else {
                                    StatusPill(text: "GYM OPEN · \(MockData.trainingNow) people training now")
                                }
                            }
                            .padding(.bottom, Theme.Space.s)
                        }

                    if state == .trainingInProgress {
                        HStack {
                            Chip(icon: "hourglass", text: "Training · 4 of 10", filled: false) {
                                app.go(.training(.running))
                            }
                            Spacer()
                        }
                    }

                    // E11
                    NextLine(text: nextLine).padding(.top, Theme.Space.xs)

                    // E03 (the one primary)
                    RowButton(icon: "dumbbell.fill", title: gymTitle, subtitle: gymSubtitle, style: .primary) {
                        app.go(.gym(state == .checkWaiting ? .checkWaiting : (state == .noRunsLeft ? .noRunsLeft : .returning)))
                    }
                    // E12
                    RowButton(icon: "message.fill", title: "Chat with OpenAgents",
                              subtitle: "Ask us anything. No setup needed.") {
                        app.go(.newChat(.returning))
                    }
                    if showLater {
                        // E06
                        RowButton(icon: "suit.spade.fill", title: "Coder",
                                  subtitle: "Score \(MockData.coderScoreAfter) of \(MockData.practiceTasks) · up 2 this week") {
                            app.go(.coder(.normal))
                        }
                        // E07
                        RowButton(icon: "trophy.fill", title: "Rankings",
                                  subtitle: "You're #\(MockData.yourRank) this week") {
                            app.go(.rankings(.week))
                        }
                    }
                    // E08
                    RowButton(icon: "person.fill", title: "Profile", subtitle: "Level, XP, help") {
                        app.go(.profile(.normal))
                    }

                    // E09
                    AnnouncementCard(title: MockData.season, line: MockData.seasonLine) {
                        app.go(showLater ? .rankings(.season) : .profile(.normal))
                    }
                    .padding(.top, Theme.Space.xs)

                    // E10
                    HStack {
                        Circle().fill(state == .offline ? Theme.Colors.statusOffline : Theme.Colors.statusLive)
                            .frame(width: 7, height: 7)
                        Text(state == .offline ? "Offline" : "Gym open")
                        Spacer()
                        Text(MockData.appVersion)
                    }
                    .font(Theme.Fonts.caption)
                    .foregroundStyle(Theme.Colors.textTertiary)
                    .padding(.vertical, Theme.Space.s)
                    .contentShape(Rectangle())
                    .onLongPressGesture { app.present(.report(.form)) }
                }
                .padding(.horizontal, Theme.Space.page)
                .padding(.top, Theme.Space.xs)
            }
            .scrollIndicators(.hidden)
        }
        .background(
            ZStack {
                Theme.Colors.background
                FlatGrid().opacity(0.8)
            }
            .ignoresSafeArea()
        )
        .foregroundStyle(Theme.Colors.textPrimary)
        .toolbar(.hidden, for: .navigationBar)
    }
}

#Preview("SCR-01 Main menu") {
    NavigationStack { SCR01MainMenu(state: .normal) }.environment(MockApp())
}

#Preview("SCR-01 Loading") {
    NavigationStack { SCR01MainMenu(state: .loading) }.environment(MockApp())
}

#Preview("SCR-01 Check waiting") {
    NavigationStack { SCR01MainMenu(state: .checkWaiting) }.environment(MockApp())
}

#Preview("SCR-01 v1 cut") {
    NavigationStack { SCR01MainMenu(state: .v1Cut) }.environment(MockApp())
}
