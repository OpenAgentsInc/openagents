import SwiftUI

// SCR-01 Main menu. The hub the player returns to. Its one primary action
// is chat (E12), because the loop happens in chat.

enum SCR01State: String, Hashable, CaseIterable {
    case normal, loading, checkWaiting, resultChecked, toolAdopted, noRunsLeft, runInProgress, offline, v1Cut
}

struct SCR01MainMenu: View {
    @Environment(MockApp.self) private var app
    let state: SCR01State

    /// The state after what the player did in this session (a run going,
    /// a result checked, no runs left); the Screen index sets one directly.
    private var live: SCR01State {
        guard state == .normal else { return state }
        if app.runningTool != nil { return .runInProgress }
        if app.checkNotice { return .resultChecked }
        if app.runsLeft == 0 { return .noRunsLeft }
        return .normal
    }

    private var showLater: Bool { MockData.showLaterFeatures && live != .v1Cut }

    // E11
    private var nextLine: String {
        switch live {
        case .checkWaiting: "a check is waiting for you (+\(MockData.xpForACheck) XP)."
        case .resultChecked: "\(MockData.checkTrainer) confirmed your result. +\(MockData.xpWhenChecked) XP."
        case .toolAdopted: "Coder now uses your tool for everyone. +\(MockData.xpWhenAdopted) XP."
        case .noRunsLeft: "new runs at \(MockData.newRunsAt) tomorrow. You can still ask what's new or check results."
        case .runInProgress: "your test is running. We'll post the result in chat."
        case .offline: "you're offline. We'll update when you're back."
        default: "test a tool to see if it makes Coder better."
        }
    }

    // E12 subtitle
    private var chatSubtitle: String {
        switch live {
        case .runInProgress: "Testing \(app.runningTool ?? MockData.defaultTool.name) now · see how it's going"
        case .checkWaiting: "A check is waiting for you"
        case .resultChecked, .toolAdopted: "See what you earned"
        default: "Test a tool, see what's new, earn XP"
        }
    }

    /// E13: Check a result goes first when one is waiting.
    private var starterChips: [String] {
        live == .checkWaiting || live == .noRunsLeft
            ? ["checkAResult", "whatsNew", "testATool"]
            : MockData.starterChips
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
                               loading: live == .loading) { app.go(.profile(.normal)) }

                    // E05
                    HeroArt()
                        .frame(height: Theme.Size.heroHeight)
                        .overlay(alignment: .bottom) {
                            Group {
                                if live == .loading {
                                    StatusPill(text: "GYM OPEN · Not known yet")
                                } else if live == .offline {
                                    StatusPill(text: "OFFLINE · Last updated 10:42", live: false)
                                } else {
                                    StatusPill(text: "GYM OPEN · \(MockData.testingNow) people testing now")
                                }
                            }
                            .padding(.bottom, Theme.Space.s)
                        }

                    // E11
                    NextLine(text: nextLine).padding(.top, Theme.Space.xs)

                    // E12 (the one primary)
                    RowButton(icon: "message.fill", title: "Chat with OpenAgents", subtitle: chatSubtitle,
                              style: .primary) { openChat() }

                    // E13: outlined starter chips; each opens a chat with that message sent.
                    HStack {
                        FlowLayout {
                            ForEach(starterChips, id: \.self) { id in
                                let a = MockData.answer(id)
                                Chip(icon: a.icon, text: a.question) { app.go(.conversation(.answer(id))) }
                            }
                        }
                        Spacer(minLength: 0)
                    }

                    if showLater {
                        // E06
                        RowButton(icon: "suit.spade.fill", title: "Coder",
                                  subtitle: "Passes \(MockData.starterPassedNow) of \(MockData.starterTests) starter tests") {
                            app.go(.coder(.normal))
                        }
                        // E07
                        RowButton(icon: "trophy.fill", title: "Rankings",
                                  subtitle: "You're #\(MockData.yourRank) this week") {
                            app.go(.rankings(.week))
                        }
                    }
                    // E08
                    RowButton(icon: "person.fill", title: "Profile", subtitle: "Level, XP, what you made") {
                        app.go(.profile(.normal))
                    }
                    if showLater {
                        // E14 (later): the Gym in the Verse, to review results on its boards.
                        RowButton(icon: "globe", title: "The Gym in the Verse",
                                  subtitle: "See every result on the boards") {
                            app.go(.stub("The Gym in the Verse"))
                        }
                    }

                    // E09
                    AnnouncementCard(title: MockData.season, line: MockData.seasonLine) {
                        app.go(showLater ? .rankings(.season) : .profile(.normal))
                    }
                    .padding(.top, Theme.Space.xs)

                    // E10
                    HStack {
                        Circle().fill(live == .offline ? Theme.Colors.statusOffline : Theme.Colors.statusLive)
                            .frame(width: 7, height: 7)
                        Text(live == .offline ? "Offline" : "Gym open")
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

    /// E12: a new chat, or the chat with the pending card on top.
    private func openChat() {
        switch live {
        case .runInProgress: app.go(.conversation(.resumeRun))
        case .checkWaiting: app.go(.conversation(.answer("checkAResult")))
        case .resultChecked, .toolAdopted:
            app.checkNotice = false
            app.go(.conversation(.answer("credit")))
        default: app.go(.newChat(.returning))
        }
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

#Preview("SCR-01 Result checked") {
    NavigationStack { SCR01MainMenu(state: .resultChecked) }.environment(MockApp())
}

#Preview("SCR-01 No runs left") {
    NavigationStack { SCR01MainMenu(state: .noRunsLeft) }.environment(MockApp())
}

#Preview("SCR-01 v1 cut") {
    NavigationStack { SCR01MainMenu(state: .v1Cut) }.environment(MockApp())
}
