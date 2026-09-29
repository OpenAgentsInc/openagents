import SwiftUI

// The hidden Screen index: long press the OPENAGENTS logo (or "STEP 1 OF 3"
// on the first screen) to list every spec ID and state and jump straight
// to it. Not part of the spec; a design tool only.

struct ScreenIndexEntry: Identifiable {
    /// "SCR-05.worse": the spec ID, a dot, and the state. Also the value
    /// for the `--screen` launch argument.
    let id: String
    let specID: String
    let name: String
    let state: String
    let open: (MockApp) -> Void
}

enum ScreenIndexCatalog {
    private static func route(_ spec: String, _ name: String, _ state: String, _ r: Route) -> ScreenIndexEntry {
        ScreenIndexEntry(id: "\(spec).\(state)", specID: spec, name: name, state: state) { $0.jump(to: r) }
    }

    private static func sheet(_ spec: String, _ name: String, _ state: String, _ s: Sheet) -> ScreenIndexEntry {
        ScreenIndexEntry(id: "\(spec).\(state)", specID: spec, name: name, state: state) { app in
            app.jump(to: .mainMenu(.normal))
            app.present(s)
        }
    }

    static let all: [ScreenIndexEntry] = {
        var e: [ScreenIndexEntry] = []
        e.append(ScreenIndexEntry(id: "FLOW-01.start", specID: "FLOW-01", name: "First-time playtester", state: "start over") {
            $0.resetDemo()
        })
        e += SCR01State.allCases.map { route("SCR-01", "Main menu", $0.rawValue, .mainMenu($0)) }
        e += SCR02State.allCases.map { s in
            ScreenIndexEntry(id: "SCR-02.\(s.rawValue)", specID: "SCR-02", name: "Choose your agent", state: s.rawValue) { app in
                app.resetDemo()
                if s != .normal { app.path = [.chooseAgent(s)] }
            }
        }
        e.append(route("CIN-01", "Intro cinematic", "play", .cinematic(0)))
        for (i, shot) in MockData.shots.enumerated() {
            e.append(route("CIN-01", "Intro cinematic", shot.id, .cinematic(i)))
        }
        e += SCR03State.allCases.map { route("SCR-03", "The Gym", $0.rawValue, .gym($0)) }
        e += SCR04State.allCases.map { route("SCR-04", "Training", $0.rawValue, .training($0)) }
        e += SCR05State.allCases.map { route("SCR-05", "Result", $0.rawValue, .result($0)) }
        e += SCR06State.allCases.map { route("SCR-06", "Level up", $0.rawValue, .levelUp($0)) }
        e += SCR07State.allCases.map { route("SCR-07", "Coder", $0.rawValue, .coder($0)) }
        e += SCR08State.allCases.map { route("SCR-08", "All tools", $0.rawValue, .allTools($0)) }
        e += MockData.tools.map { route("SCR-09", "Tool detail", $0.id, .toolDetail($0.id)) }
        e += SCR10State.allCases.map { route("SCR-10", "Rankings", $0.rawValue, .rankings($0)) }
        e += SCR11State.allCases.map { route("SCR-11", "Profile", $0.rawValue, .profile($0)) }
        e += SCR12State.allCases.map { route("SCR-12", "Updates", $0.rawValue, .updates($0)) }
        e += SCR13State.allCases.map { sheet("SCR-13", "Report a problem", $0.rawValue, .report($0)) }
        e += SCR14State.allCases.map { sheet("SCR-14", "Setup prompt", $0.rawValue, .setup($0)) }
        e += SCR15State.allCases.map { route("SCR-15", "Chat: new chat", $0.rawValue, .newChat($0)) }
        e += SCR16State.allCases.map { route("SCR-16", "Chat: previous chats", $0.rawValue, .previousChats($0)) }
        let conversations: [(String, SCR17State)] = [
            ("prepared", .answer("can")), ("whoAreYou", .answer("who")), ("streaming", .freeText("How does the Gym measure a tool?")),
            ("thinking", .thinking), ("failed", .failed), ("dispatch", .answer("fix")), ("afterRunCoder", .afterRunCoder),
            ("noComputer", .noComputer), ("aboutResult", .aboutResult), ("aboutTool", .aboutTool), ("goToGym", .answer("next")),
            ("screenChip", .answer("wallet")),
        ]
        e += conversations.map { route("SCR-17", "Chat: a conversation", $0.0, .conversation($0.1)) }
        e += SCR18State.allCases.map { route("SCR-18", "Chat: Wrong answer", $0.rawValue, .conversation(.wrongAnswer($0))) }
        e += SCR19State.allCases.map { route("SCR-19", "Chat: Coder on a computer", $0.rawValue, .coderChat($0)) }
        e += PAT01State.allCases.map { route("PAT-01", "Offline, error, empty", $0.rawValue, .pattern($0)) }
        return e
    }()
}

struct ScreenIndex: View {
    @Environment(MockApp.self) private var app

    private var groups: [(String, [ScreenIndexEntry])] {
        var order: [String] = []
        var map: [String: [ScreenIndexEntry]] = [:]
        for e in ScreenIndexCatalog.all {
            if map[e.specID] == nil { order.append(e.specID) }
            map[e.specID, default: []].append(e)
        }
        return order.map { ($0, map[$0]!) }
    }

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Screen index").condensedTitle(Theme.Fonts.screenTitle, tracking: 1.2)
                Spacer()
                Button("Close") { app.showIndex = false }.font(Theme.Fonts.bodyBold)
            }
            .padding(.horizontal, Theme.Space.page)
            .frame(height: Theme.Size.topBarHeight)
            ScrollView {
                VStack(alignment: .leading, spacing: Theme.Space.m) {
                    Text("Tap a state to open it. Debug only; not in the spec.")
                        .font(Theme.Fonts.caption).foregroundStyle(Theme.Colors.textTertiary)
                    ForEach(groups, id: \.0) { spec, entries in
                        VStack(alignment: .leading, spacing: 8) {
                            HStack(spacing: 8) {
                                Text(spec).font(Theme.Fonts.captionMono).foregroundStyle(Theme.Colors.textSecondary)
                                Text(entries[0].name).font(Theme.Fonts.bodyBold)
                            }
                            FlowLayout(spacing: 6) {
                                ForEach(entries) { entry in
                                    Chip(text: entry.state) { entry.open(app) }
                                }
                            }
                        }
                    }
                }
                .padding(.horizontal, Theme.Space.page)
                .padding(.bottom, Theme.Space.xxl)
            }
        }
        .foregroundStyle(Theme.Colors.textPrimary)
        .background(Theme.Colors.scrim.background(.ultraThinMaterial).ignoresSafeArea())
    }
}

#Preview("Screen index") {
    ScreenIndex().environment(MockApp())
}
