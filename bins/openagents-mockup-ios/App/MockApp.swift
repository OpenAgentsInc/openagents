import SwiftUI

// Navigation for the mockup: one stack of routes over a root screen.
// The root is SCR-02 on the first run and SCR-01 after it (the spec's
// NAV-01: the main menu is the hub; there is no tab bar, and chat is the
// hub's primary). Every route names its spec ID and the state it shows,
// so the Screen index can jump straight to any screen in any state.

enum Route: Hashable {
    case mainMenu(SCR01State)
    case chooseAgent(SCR02State)
    case cinematic(Int)   // the shot to start at
    case result(SCR05State, UUID?)   // the chat message whose CARD-04 opened it
    case levelUp(SCR06State)
    case coder(SCR07State)
    case allTools(SCR08State)
    case toolDetail(String)
    case rankings(SCR10State)
    case profile(SCR11State)
    case updates(SCR12State)
    case newChat(SCR15State)
    case previousChats(SCR16State)
    case conversation(SCR17State)
    case coderChat(SCR19State)
    case pattern(PAT01State)
    /// Retired in revision 3; reachable only from the Screen index.
    case retiredGym(SCR03State)
    case retiredTraining(SCR04State)
    /// A screen outside the spec (Wallet, Computers, …): a labeled placeholder.
    case stub(String)
}

enum Sheet: Identifiable, Hashable {
    case report(SCR13State)
    case setup(SCR14State)
    /// SCR-20: the outcome key, and the chat message whose result it adds.
    case addToGym(SCR20State, String, UUID?)
    /// SCR-21: a test set id, and whether it's the player's draft.
    case testSet(String, draft: Bool)
    var id: Self { self }
}

/// FLOW-01's furthest step. Kept across relaunch, so the app reopens where
/// the player left off (spec FLOW-01 rule 1).
enum FirstRunStep: Int {
    case chooseAgent, cinematic, chat, running, result, done
}

/// Something a sheet or screen asks the open chat to do (the chat owns its
/// messages; this is how SCR-05, SCR-20, and SCR-21 reach back into it).
enum ChatCommand: Equatable {
    case added(UUID)
    case runFullTestSet(UUID?)
    case approveDraft
    case send(String)          // an answer id
    case checkedByOther
}

@Observable
final class MockApp {
    var path: [Route] = []
    var firstRunComplete = false
    var menuState: SCR01State = .normal
    var sheet: Sheet?
    var showIndex = false

    // Fake player state that the loop changes.
    var level = MockData.player.level
    var xp = MockData.player.xp
    var xpForNextLevel = MockData.player.xpForNextLevel
    var runsLeft = MockData.runsLeftToday
    var resultsAdded = 0
    /// A run in progress (the tool's name), for the SCR-01.E12 subtitle.
    var runningTool: String?
    /// Another trainer confirmed your result (SCR-01.E11, FLOW-10).
    var checkNotice = false
    /// Commands for the open chat, and which chat is open.
    var inbox: [ChatCommand] = []
    var activeChat: UUID?

    // FLOW-01, kept across relaunch.
    var firstRunStep: FirstRunStep {
        didSet { UserDefaults.standard.set(firstRunStep.rawValue, forKey: Keys.step) }
    }
    var runStartedAt: Date? {
        didSet { UserDefaults.standard.set(runStartedAt?.timeIntervalSince1970 ?? 0, forKey: Keys.started) }
    }

    private enum Keys {
        static let step = "oa.first-run-step"
        static let started = "oa.run-started-at"
    }

    init() {
        let d = UserDefaults.standard
        firstRunStep = FirstRunStep(rawValue: d.integer(forKey: Keys.step)) ?? .chooseAgent
        let t = d.double(forKey: Keys.started)
        runStartedAt = t > 0 ? Date(timeIntervalSince1970: t) : nil
        firstRunComplete = firstRunStep == .done
    }

    var isFirstRun: Bool { firstRunStep != .done }

    func go(_ route: Route) {
        withAnimation(Theme.Motion.screen) { path.append(route) }
    }

    func back() {
        if !path.isEmpty { path.removeLast() }
    }

    func backToMenu() {
        if firstRunStep != .done { firstRunStep = .done }
        firstRunComplete = true
        menuState = .normal
        path.removeAll()
    }

    /// Reopen FLOW-01 at the furthest step reached (app launch).
    func resume() {
        switch firstRunStep {
        case .chooseAgent, .done: break
        case .cinematic: path = [.cinematic(MockData.shots.count - 1)]
        case .chat, .running, .result: path = [.conversation(.firstRun)]
        }
    }

    /// Replace the stack (used by the Screen index and launch arguments).
    func jump(to route: Route) {
        showIndex = false
        sheet = nil
        if case .chooseAgent = route {
            resetDemo()
            return
        }
        firstRunComplete = true
        if case .mainMenu(let s) = route {
            menuState = s
            path = []
        } else {
            menuState = .normal
            path = [route]
        }
    }

    func present(_ sheet: Sheet) {
        showIndex = false
        self.sheet = sheet
    }

    // MARK: The chat mailbox

    func post(_ command: ChatCommand) { inbox.append(command) }

    func take() -> [ChatCommand] {
        let c = inbox
        inbox = []
        return c
    }

    /// Back to the chat under this screen and send a message there (SCR-05
    /// "Test another tool", "Ask about this result"). With no chat under
    /// it, open a new one.
    func returnToChat(sending answerID: String? = nil, command: ChatCommand? = nil) {
        if let i = path.dropLast().lastIndex(where: { if case .conversation = $0 { true } else { false } }) {
            path.removeSubrange((i + 1)...)
            if let command { post(command) }
            if let answerID { post(.send(answerID)) }
        } else if let answerID {
            path = path.dropLast() + [.conversation(.answer(answerID))]
        } else {
            back()
        }
    }

    // MARK: The loop

    /// START THE TEST: spends one of today's runs (checks and first tries don't).
    func startRun(tool: String, spendsRun: Bool) {
        if spendsRun && runsLeft > 0 { runsLeft -= 1 }
        runningTool = tool
    }

    func stopRun(refund: Bool) {
        if refund { runsLeft = min(MockData.runsLeftToday, runsLeft + 1) }
        runningTool = nil
    }

    /// SCR-20 ADD TO THE GYM. Returns true when SCR-06 should show.
    func confirmAdd(outcome key: String, messageID: UUID?) -> Bool {
        let outcome = MockData.outcome(key)
        resultsAdded += 1
        if let messageID { post(.added(messageID)) }
        // FLOW-10: another trainer checks it a few seconds later (simulated),
        // or our referee confirms your check.
        Task { @MainActor in
            try? await Task.sleep(for: .seconds(MockData.fakeCheckSeconds))
            self.award(outcome.isCheck ? MockData.xpForACheck : MockData.xpWhenChecked)
            if !outcome.isCheck {
                self.checkNotice = true
                self.post(.checkedByOther)
            }
        }
        if MockData.levelUpOnFirstAdd && resultsAdded == 1 && level == MockData.player.level {
            level += 1
            xp = 40
            xpForNextLevel = Int(Double(xpForNextLevel) * 1.4)
            return true
        }
        return false
    }

    private func award(_ n: Int) {
        xp += n
        if xp >= xpForNextLevel {
            xp -= xpForNextLevel
            level += 1
            xpForNextLevel = Int(Double(xpForNextLevel) * 1.4)
        }
    }

    /// After SCR-20 closes: Level up, or (in the first run) the main menu.
    func afterAdd(levelUp: Bool, firstRun: Bool) {
        sheet = nil
        Task { @MainActor in
            try? await Task.sleep(for: .seconds(0.45))
            if levelUp { self.go(.levelUp(.withTitle)) } else if firstRun { self.backToMenu() }
        }
    }

    func resetDemo() {
        path = []
        sheet = nil
        firstRunStep = .chooseAgent
        runStartedAt = nil
        firstRunComplete = false
        menuState = .normal
        level = MockData.player.level
        xp = MockData.player.xp
        xpForNextLevel = MockData.player.xpForNextLevel
        runsLeft = MockData.runsLeftToday
        resultsAdded = 0
        runningTool = nil
        checkNotice = false
        inbox = []
        showIndex = false
    }
}

@main
struct OpenAgentsMockupApp: App {
    @State private var app = MockApp()

    init() { PaperMono.installAppearance() }

    var body: some Scene {
        WindowGroup {
            RootView()
                .font(.paper(.body))
                .environment(app)
                .preferredColorScheme(.dark)
                .onAppear { if !LaunchArguments.apply(to: app) { app.resume() } }
        }
    }
}

struct RootView: View {
    @Environment(MockApp.self) private var app

    var body: some View {
        @Bindable var app = app
        NavigationStack(path: $app.path) {
            Group {
                if app.firstRunComplete {
                    SCR01MainMenu(state: app.menuState)
                } else {
                    SCR02ChooseAgent(state: .normal)
                }
            }
            .navigationDestination(for: Route.self) { route in
                RouteView(route: route)
            }
        }
        .tint(Theme.Colors.textPrimary)
        .sheet(item: $app.sheet) { sheet in
            Group {
                switch sheet {
                case .report(let s): SCR13ReportProblem(state: s)
                case .setup(let s): SCR14SetupPrompt(state: s)
                case .addToGym(let s, let key, let id): SCR20AddToGym(state: s, outcomeKey: key, messageID: id)
                case .testSet(let id, let draft): SCR21TestSet(setID: id, draft: draft)
                }
            }
            .presentationDetents(sheetDetents(sheet))
            .presentationBackground(Theme.Colors.surface)
            .presentationCornerRadius(Theme.Radius.sheet)
        }
        .overlay {
            if app.showIndex {
                ScreenIndex()
                    .transition(.opacity)
            }
        }
        .animation(Theme.Motion.screen, value: app.showIndex)
    }

    private func sheetDetents(_ sheet: Sheet) -> Set<PresentationDetent> {
        switch sheet {
        case .testSet: [.large]
        case .addToGym: [.fraction(Theme.Size.addToGymSheet), .large]
        default: [.medium, .large]
        }
    }
}

/// Maps a route to its screen. One line per spec screen.
struct RouteView: View {
    let route: Route

    var body: some View {
        Group {
            switch route {
            case .mainMenu(let s): SCR01MainMenu(state: s)
            case .chooseAgent(let s): SCR02ChooseAgent(state: s)
            case .cinematic(let shot): CIN01IntroCinematic(startAt: shot)
            case .result(let s, let id): SCR05Result(state: s, messageID: id)
            case .levelUp(let s): SCR06LevelUp(state: s)
            case .coder(let s): SCR07Coder(state: s)
            case .allTools(let s): SCR08AllTools(state: s)
            case .toolDetail(let id): SCR09ToolDetail(toolID: id)
            case .rankings(let s): SCR10Rankings(state: s)
            case .profile(let s): SCR11Profile(state: s)
            case .updates(let s): SCR12Updates(state: s)
            case .newChat(let s): SCR15NewChat(state: s)
            case .previousChats(let s): SCR16PreviousChats(state: s)
            case .conversation(let s): SCR17Conversation(state: s)
            case .coderChat(let s): SCR19CoderOnComputer(state: s)
            case .pattern(let s): PAT01States(state: s)
            case .retiredGym(let s): SCR03Gym(state: s)
            case .retiredTraining(let s): SCR04Training(state: s)
            case .stub(let name): StubScreen(name: name)
            }
        }
        .toolbar(.hidden, for: .navigationBar)
    }
}

/// `--screen <index id>` opens a screen from the Screen index at launch,
/// e.g. `--screen SCR-05.worse` (used by build.sh shots for screenshots).
enum LaunchArguments {
    /// Returns true when a `--screen` argument was applied.
    static func apply(to app: MockApp) -> Bool {
        let args = ProcessInfo.processInfo.arguments
        guard let i = args.firstIndex(of: "--screen"), i + 1 < args.count else { return false }
        let id = args[i + 1]
        guard let entry = ScreenIndexCatalog.all.first(where: { $0.id == id }) else { return false }
        entry.open(app)
        return true
    }
}
