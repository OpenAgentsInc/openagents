import SwiftUI

// Navigation for the mockup: one stack of routes over a root screen.
// The root is SCR-02 on the first run and SCR-01 after it (the spec's
// NAV-01: the main menu is the hub; there is no tab bar).
// Every route names its spec ID and the state it shows, so the Screen
// index can jump straight to any screen in any state.

enum Route: Hashable {
    case mainMenu(SCR01State)
    case chooseAgent(SCR02State)
    case cinematic(Int)   // the shot to start at
    case gym(SCR03State)
    case training(SCR04State)
    case result(SCR05State)
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
    /// A screen outside the spec (Wallet, Computers, …): a labeled placeholder.
    case stub(String)
}

enum Sheet: Identifiable, Hashable {
    case report(SCR13State)
    case setup(SCR14State)
    var id: Self { self }
}

@Observable
final class MockApp {
    var path: [Route] = []
    var firstRunComplete = false
    var sheet: Sheet?
    var showIndex = false

    // Fake player state that the loop changes.
    var level = MockData.player.level
    var xp = MockData.player.xp
    var xpForNextLevel = MockData.player.xpForNextLevel
    var runsLeft = MockData.runsLeftToday
    var selectedToolID = MockData.defaultTool.id
    var resultAdded = false
    var trainingFinished = false

    var selectedTool: MockData.Tool { MockData.tool(selectedToolID) }
    var isFirstRun: Bool { !firstRunComplete }

    func go(_ route: Route) {
        withAnimation(Theme.Motion.screen) { path.append(route) }
    }

    func back() {
        if !path.isEmpty { path.removeLast() }
    }

    func backToMenu() {
        firstRunComplete = true
        path.removeAll()
    }

    /// Replace the stack (used by the Screen index and launch arguments).
    func jump(to route: Route) {
        showIndex = false
        sheet = nil
        if case .chooseAgent = route {
            firstRunComplete = false
            path = []
            return
        }
        firstRunComplete = true
        if case .mainMenu(.normal) = route { path = [] } else { path = [route] }
    }

    func present(_ sheet: Sheet) {
        showIndex = false
        self.sheet = sheet
    }

    /// Starting a run spends one of today's runs (checks don't).
    func startRun() {
        if runsLeft > 0 { runsLeft -= 1 }
        trainingFinished = false
        resultAdded = false
        go(.training(isFirstRun ? .firstRun : .running))
    }

    /// ADD MY RESULT TO THE GYM: adds XP; returns true when a level was crossed.
    func addResult() -> Bool {
        resultAdded = true
        xp += MockData.xpPerRun
        if xp >= xpForNextLevel || MockData.levelUpOnFirstAdd && level == MockData.player.level {
            level += 1
            xp = max(0, xp - xpForNextLevel)
            xpForNextLevel = Int(Double(xpForNextLevel) * 1.4)
            return true
        }
        return false
    }

    func resetDemo() {
        path = []
        sheet = nil
        firstRunComplete = false
        level = MockData.player.level
        xp = MockData.player.xp
        xpForNextLevel = MockData.player.xpForNextLevel
        runsLeft = MockData.runsLeftToday
        selectedToolID = MockData.defaultTool.id
        resultAdded = false
        trainingFinished = false
        showIndex = false
    }
}

@main
struct OpenAgentsMockupApp: App {
    @State private var app = MockApp()

    var body: some Scene {
        WindowGroup {
            RootView()
                .environment(app)
                .preferredColorScheme(.dark)
                .onAppear { LaunchArguments.apply(to: app) }
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
                    SCR01MainMenu(state: .normal)
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
                }
            }
            .presentationDetents([.medium, .large])
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
            case .gym(let s): SCR03Gym(state: s)
            case .training(let s): SCR04Training(state: s)
            case .result(let s): SCR05Result(state: s)
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
            case .stub(let name): StubScreen(name: name)
            }
        }
        .toolbar(.hidden, for: .navigationBar)
    }
}

/// `--screen <index id>` opens a screen from the Screen index at launch,
/// e.g. `--screen SCR-05.worse` (used by build.sh shots for screenshots).
enum LaunchArguments {
    static func apply(to app: MockApp) {
        let args = ProcessInfo.processInfo.arguments
        guard let i = args.firstIndex(of: "--screen"), i + 1 < args.count else { return }
        let id = args[i + 1]
        guard let entry = ScreenIndexCatalog.all.first(where: { $0.id == id }) else { return }
        entry.open(app)
    }
}
