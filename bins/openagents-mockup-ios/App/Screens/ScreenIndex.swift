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
    /// The index's section: Flows, Screens, Chat cards, Sheets, Retired (rev 2).
    var group = ScreenIndexCatalog.screens
    let open: (MockApp) -> Void
}

enum ScreenIndexCatalog {
    static let flows = "Flows · start over"
    static let screens = "Screens"
    static let cards = "Chat cards"
    static let sheets = "Sheets"
    static let retired = "Retired (rev 2)"

    private static func route(_ spec: String, _ name: String, _ state: String, _ r: Route,
                              group: String = screens) -> ScreenIndexEntry {
        ScreenIndexEntry(id: "\(spec).\(state)", specID: spec, name: name, state: state, group: group) { $0.jump(to: r) }
    }

    private static func sheet(_ spec: String, _ name: String, _ state: String, _ s: Sheet,
                              group: String = screens) -> ScreenIndexEntry {
        ScreenIndexEntry(id: "\(spec).\(state)", specID: spec, name: name, state: state, group: group) { app in
            app.jump(to: .mainMenu(.normal))
            app.present(s)
        }
    }

    private static func card(_ spec: String, _ name: String, _ state: String, _ c: ChatCard) -> ScreenIndexEntry {
        route(spec, name, state, .conversation(.card(c)), group: cards)
    }

    private static func flow(_ n: Int, _ name: String, _ start: @escaping (MockApp) -> Void) -> ScreenIndexEntry {
        let spec = String(format: "FLOW-%02d", n)
        return ScreenIndexEntry(id: "\(spec).start", specID: spec, name: name, state: "start over", group: flows) { app in
            app.resetDemo()
            if n != 1 { app.firstRunStep = .done; app.firstRunComplete = true }
            start(app)
        }
    }

    static let all: [ScreenIndexEntry] = {
        var e: [ScreenIndexEntry] = []

        // Flows: each starts over from where the flow begins.
        e.append(flow(1, "First-time playtester") { _ in })
        e.append(flow(2, "Returning playtester, daily loop") { _ in })
        e.append(flow(3, "Kicking the tires in chat") { $0.path = [.newChat(.firstTime)] })
        e.append(flow(4, "Test a tool from chat") { $0.path = [.conversation(.answer("whichTool"))] })
        e.append(flow(5, "Dispatching Coder from chat") { $0.path = [.conversation(.answer("fix"))] })
        e.append(flow(6, "Wrong answer and Share this chat") { $0.path = [.conversation(.answer("can"))] })
        e.append(flow(7, "Make a tool and its tests in chat") { $0.path = [.conversation(.answer("makeTool"))] })
        e.append(flow(8, "Check another trainer's result") { $0.menuState = .checkWaiting })
        e.append(flow(9, "What's new in the Gym") { $0.path = [.conversation(.answer("whatsNew"))] })
        e.append(flow(10, "Credit: when your work is used") { $0.menuState = .resultChecked })

        // Screens.
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
        e += SCR05State.allCases.map { route("SCR-05", "Result (detail)", $0.rawValue, .result($0, nil)) }
        e += SCR06State.allCases.map { route("SCR-06", "Level up", $0.rawValue, .levelUp($0)) }
        e += SCR07State.allCases.map { route("SCR-07", "Coder", $0.rawValue, .coder($0)) }
        e += SCR08State.allCases.map { route("SCR-08", "All tools", $0.rawValue, .allTools($0)) }
        e += MockData.tools.map { route("SCR-09", "Tool detail", $0.id, .toolDetail($0.id)) }
        e += SCR10State.allCases.map { route("SCR-10", "Rankings", $0.rawValue, .rankings($0)) }
        e += SCR11State.allCases.map { route("SCR-11", "Profile", $0.rawValue, .profile($0)) }
        e += SCR12State.allCases.map { route("SCR-12", "Updates", $0.rawValue, .updates($0)) }
        e += SCR13State.allCases.map { sheet("SCR-13", "Report a problem", $0.rawValue, .report($0)) }
        e += SCR14State.allCases.map { sheet("SCR-14", "Setup prompt", $0.rawValue, .setup($0)) }
        e.append(route("SCR-15", "Chat: new chat", "firstRun", .conversation(.firstRun)))
        e += SCR15State.allCases.map { route("SCR-15", "Chat: new chat", $0.rawValue, .newChat($0)) }
        e += SCR16State.allCases.map { route("SCR-16", "Chat: previous chats", $0.rawValue, .previousChats($0)) }
        let conversations: [(String, SCR17State)] = [
            ("prepared", .answer("can")), ("whoAreYou", .answer("who")), ("streaming", .freeText("How does the Gym measure a tool?")),
            ("thinking", .thinking), ("failed", .failed), ("dispatch", .answer("fix")), ("afterRunCoder", .afterRunCoder),
            ("noComputer", .noComputer), ("aboutResult", .answer("result")), ("aboutTool", .answer("tool")),
            ("whatsTheGym", .answer("gym")), ("screenChip", .answer("wallet")), ("testATool", .answer("testATool")),
            ("makeATool", .answer("makeTool")), ("howDidMyTestDo", .answer("howDid")), ("earnXP", .answer("xp")),
        ]
        e += conversations.map { route("SCR-17", "Chat: a conversation", $0.0, .conversation($0.1)) }
        e += SCR18State.allCases.map { route("SCR-18", "Chat: Wrong answer", $0.rawValue, .conversation(.wrongAnswer($0))) }
        e += SCR19State.allCases.map { route("SCR-19", "Chat: Coder on a computer", $0.rawValue, .coderChat($0)) }
        e += PAT01State.allCases.map { route("PAT-01", "Offline, error, empty", $0.rawValue, .pattern($0)) }

        // Chat cards, each in a conversation that asked for it.
        e.append(card("CARD-01", "Tool", "ready", .tool("project-map", .ready)))
        e.append(card("CARD-01", "Tool", "notTestedYet", .tool("test-reader", .ready)))
        e.append(card("CARD-01", "Tool", "noRunsLeft", .tool("project-map", .noRunsLeft)))
        e.append(card("CARD-01", "Tool", "offline", .tool("project-map", .offline)))
        e += CARD02Step.allCases.map { card("CARD-02", "Test set draft", $0.rawValue, .draft($0)) }
        e += CARD03State.allCases.map { card("CARD-03", "Run", $0.rawValue, .run(.tool("project-map"), $0, Date())) }
        e.append(card("CARD-03", "Run", "checking", .run(.check, .running, Date())))
        e.append(card("CARD-03", "Run", "tryOnce", .run(.tryOnce, .running, Date())))
        e += CARD04State.allCases.map { card("CARD-04", "Result", $0.rawValue, .result($0.outcomeKey, added: $0 == .added)) }
        e += CARD05State.allCases.map { card("CARD-05", "Gym news", $0.rawValue, .news($0)) }
        e += CARD06State.allCases.map { card("CARD-06", "Check", $0.rawValue, .check($0)) }
        e += CARD07State.allCases.map { card("CARD-07", "Credit", $0.rawValue, .credit($0)) }

        // Sheets.
        e += SCR20State.allCases.map { sheet("SCR-20", "Add to the Gym", $0.rawValue, .addToGym($0, "better", nil), group: sheets) }
        e.append(sheet("SCR-20", "Add to the Gym", "check", .addToGym(.normal, "confirmed", nil), group: sheets))
        e.append(sheet("SCR-20", "Add to the Gym", "yourTool", .addToGym(.normal, "madeBetter", nil), group: sheets))
        e.append(sheet("SCR-21", "Test set", "readOnly", .testSet("project-map", draft: false), group: sheets))
        e.append(sheet("SCR-21", "Test set", "draft", .testSet("changelog", draft: true), group: sheets))
        e.append(sheet("SCR-21", "Test set", "anotherTrainer", .testSet("test-reader-2px", draft: false), group: sheets))

        // Retired in revision 3, kept so old findings can be compared.
        e += SCR03State.allCases.map { route("SCR-03", "The Gym (retired)", $0.rawValue, .retiredGym($0), group: retired) }
        e += SCR04State.allCases.map { route("SCR-04", "Training (retired)", $0.rawValue, .retiredTraining($0), group: retired) }
        return e
    }()
}

struct ScreenIndex: View {
    @Environment(MockApp.self) private var app

    /// Sections, each with its spec IDs in order, each with its states.
    private var sections: [(String, [(String, [ScreenIndexEntry])])] {
        var sectionOrder: [String] = []
        var bySection: [String: [ScreenIndexEntry]] = [:]
        for e in ScreenIndexCatalog.all {
            if bySection[e.group] == nil { sectionOrder.append(e.group) }
            bySection[e.group, default: []].append(e)
        }
        return sectionOrder.map { section in
            var order: [String] = []
            var map: [String: [ScreenIndexEntry]] = [:]
            for e in bySection[section]! {
                let key = section == ScreenIndexCatalog.flows ? e.id : e.specID
                if map[key] == nil { order.append(key) }
                map[key, default: []].append(e)
            }
            return (section, order.map { ($0, map[$0]!) })
        }
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
                    ForEach(sections, id: \.0) { section, groups in
                        SectionLabel(text: section).padding(.top, Theme.Space.s)
                        if section == ScreenIndexCatalog.flows {
                            VStack(alignment: .leading, spacing: 6) {
                                ForEach(groups, id: \.0) { _, entries in
                                    Chip(text: "\(entries[0].specID) · \(entries[0].name)") { entries[0].open(app) }
                                }
                            }
                        } else {
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
