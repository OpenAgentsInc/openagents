import SwiftUI

// MOCK DATA: every fake number, name, and line of copy in the mockup.
//
// Screens read their words from here, so a copy change is one edit.
// Words follow the spec's "Words on screen" rules: plain words only, no
// jargon on primary surfaces. Spec: docs/product/2026-09-28-app-wireframe.md.

enum MockData {

    // MARK: Switches for the designer

    /// Show the "later" parts of the main menu (bell, CODER and RANKINGS rows).
    /// The spec's v1 cut hides them; the Screen index also has a "v1 cut" state.
    static let showLaterFeatures = true
    /// The owner's reference shows a ₿ balance pill on the player card.
    /// The spec cuts it from v1 (no money on the main menu), so it is off.
    static let showBalancePill = false
    /// Show Skip during the first play of the cinematic too. The spec says
    /// the first play plays through; this is on so reviewing is quick.
    static let cinematicSkipAlways = true
    /// 1.0 plays the cinematic at the spec's durations (about 57 s).
    static let cinematicTimeScale: Double = 0.5
    /// Show SCR-06 Level up the first time a result is added, even though
    /// 140 + 50 XP doesn't reach level 3, so the whole FLOW-01 is visible.
    static let levelUpOnFirstAdd = true
    /// How long the fake training run takes, in seconds (the real one is ~5 min).
    static let fakeTrainingSeconds: Double = 12
    /// Delay before a fake chat reply starts, and time per streamed word.
    static let fakeReplyDelay: Double = 0.7
    static let fakeStreamWordInterval: Double = 0.045

    // MARK: Player

    struct Player {
        var name: String
        var level: Int
        var xp: Int
        var xpForNextLevel: Int
        var title: String
        var balance: String
    }

    static let player = Player(name: "Trainer 7KQ", level: 2, xp: 140, xpForNextLevel: 283,
                               title: "PLAYTESTER", balance: "₿ 0.00012")
    /// XP one Gym run or check earns.
    static let xpPerRun = 50
    static let runsLeftToday = 3
    static let trainingNow = 38
    static let appVersion = "v0.1.0 · Mockup"
    static let season = "PLAYTEST SEASON 1 · ENDS OCT 26"
    static let seasonLine = "Help make Coder better. Every run counts."

    // MARK: Coder

    static let coderScoreBefore = 6
    static let coderScoreAfter = 8
    static let practiceTasks = 10
    static let coderScoreHistory = [6, 7, 8]
    static let coderScoreWeeks = ["Sep 14", "Sep 21", "Sep 28"]

    // MARK: Tools

    struct Tool: Identifiable, Hashable {
        let id: String
        let name: String
        let icon: String          // SF Symbol name
        let line: String          // one plain sentence
        let triedBy: Int
        let status: String        // Helps / Testing / Doesn't help
        let pooledBefore: Int
        let pooledAfter: Int
        let runs: Int
        let confirmedBy: Int
    }

    static let tools: [Tool] = [
        Tool(id: "project-map", name: "Project map", icon: "map",
             line: "Shows Coder how the project is laid out before it starts.",
             triedBy: 18, status: "Helps", pooledBefore: 6, pooledAfter: 8, runs: 41, confirmedBy: 6),
        Tool(id: "code-finder", name: "Code finder", icon: "magnifyingglass",
             line: "Finds the right lines of code.",
             triedBy: 9, status: "Testing", pooledBefore: 6, pooledAfter: 7, runs: 14, confirmedBy: 2),
        Tool(id: "test-reader", name: "Test reader", icon: "checkmark.seal",
             line: "Reads test failures for Coder.",
             triedBy: 5, status: "Testing", pooledBefore: 6, pooledAfter: 6, runs: 8, confirmedBy: 0),
    ]
    static var defaultTool: Tool { tools[0] }
    static func tool(_ id: String) -> Tool { tools.first { $0.id == id } ?? tools[0] }

    /// The check card on SCR-03 (another trainer's result to confirm).
    static let checkTrainer = "Trainer 2PX"
    static let checkTool = "Code finder"

    // MARK: Runs (SCR-11 Your runs)

    struct Run: Identifiable {
        let id = UUID()
        let tool: String
        let before: Int
        let after: Int
        let verdict: String
        let xp: Int
    }

    static let runs: [Run] = [
        Run(tool: "Project map", before: 6, after: 8, verdict: "Better", xp: 50),
        Run(tool: "Code finder", before: 6, after: 6, verdict: "No change", xp: 50),
    ]

    // MARK: Rankings (SCR-10)

    struct Rank: Identifiable {
        var id: Int { place }
        let place: Int
        let name: String
        let level: Int
        let xp: Int
    }

    static let rankingsWeek: [Rank] = [
        Rank(place: 1, name: "Trainer 2PX", level: 9, xp: 1240),
        Rank(place: 2, name: "Trainer QA4", level: 8, xp: 1100),
        Rank(place: 3, name: "Trainer M0Z", level: 8, xp: 1015),
        Rank(place: 4, name: "Trainer 9LT", level: 7, xp: 880),
        Rank(place: 5, name: "Trainer K2C", level: 6, xp: 760),
        Rank(place: 6, name: "Trainer RR8", level: 6, xp: 702),
        Rank(place: 40, name: "Trainer H5J", level: 2, xp: 202),
    ]
    static let rankingsSeason: [Rank] = [
        Rank(place: 1, name: "Trainer QA4", level: 14, xp: 5210),
        Rank(place: 2, name: "Trainer 2PX", level: 13, xp: 4980),
        Rank(place: 3, name: "Trainer 9LT", level: 11, xp: 3900),
        Rank(place: 4, name: "Trainer M0Z", level: 10, xp: 3420),
    ]
    static let yourRank = 41
    static let xpToPassNext = 12

    // MARK: Updates (SCR-12)

    struct Update: Identifiable {
        let id = UUID()
        let unread: Bool
        let text: String
        let button: String
    }

    static let updates: [Update] = [
        Update(unread: true, text: "Trainer 2PX confirmed your result. +50 XP is yours.", button: "SEE IT"),
        Update(unread: true, text: "Coder now uses Project map for everyone. You helped.", button: "SEE IT"),
        Update(unread: false, text: "A check is waiting for you.", button: "CHECK"),
    ]

    // MARK: Chat

    static let computerName = "Studio Mac"
    static let workspaceName = "openagents"

    /// A tap-able offer under a reply. The app decides what a tap does,
    /// never the reply's words.
    enum Offer: Hashable {
        case runCoder
        case openCoder
        case connectComputer
        case screen(String)          // Open Wallet, Your computers, Identity keys, Playtest, Report a problem
        case goToGym
        case trainWithTool
        case startTraining
        case seeResult
        case enterGym
    }

    struct CommandCard: Hashable {
        let command: String
        let note: String
        let output: String
    }

    /// A reply the mockup can give. Chips and follow-ups point at one by id,
    /// so nothing here guesses what a typed message means.
    struct Answer: Hashable {
        let id: String
        let question: String
        let reply: String
        var prepared = true
        var offers: [Offer] = []
        var followUps: [String] = []   // Answer ids
        var command: CommandCard? = nil
        var resultLine: String? = nil
    }

    static let answers: [Answer] = [
        Answer(id: "who", question: "Who are you?",
               reply: "We're OpenAgents. We answer questions here, and when a job needs a computer, we send Coder, our AI that writes code, to do it.",
               followUps: ["can", "cost"]),
        Answer(id: "can", question: "What can you do here?",
               reply: "We answer questions, explain things, and help you plan and write. When something needs a computer, we dispatch Coder to one of yours.",
               offers: [.runCoder, .screen("Open Wallet")],
               followUps: ["model", "cost"],
               command: CommandCard(command: "openagents computer list",
                                    note: "Reads only. Runs on this phone.",
                                    output: "Studio Mac     online   openagents\nLaptop         offline  —")),
        Answer(id: "cost", question: "What does it cost?",
               reply: "Chatting with us is free. Training Coder in the Gym is free too: we pay for the runs.",
               followUps: ["gym", "xp"]),
        Answer(id: "model", question: "What model is this?",
               reply: "Replies come from Gemini 3.8 Flash through our AI Gateway. Common questions get a reviewed answer at once.",
               followUps: ["who"]),
        Answer(id: "gym", question: "What's the Gym?",
               reply: "The Gym is where you make Coder better. You give it a new tool, we test it on 10 practice tasks, and you see if its score went up.",
               offers: [.goToGym], followUps: ["next", "xp"]),
        Answer(id: "coder", question: "What is Coder?",
               reply: "Coder is our AI that writes code. You train it in the Gym, and every confirmed improvement goes to everyone's Coder.",
               offers: [.goToGym], followUps: ["gym"]),
        Answer(id: "next", question: "What should I try next?",
               reply: "Give Coder Project map. 18 trainers tried it; most saw Coder do better.",
               prepared: false, offers: [.goToGym]),
        Answer(id: "tool", question: "What does Project map do?",
               reply: "Project map shows Coder how the project is laid out before it starts, so it spends less time looking around.",
               prepared: false, offers: [.trainWithTool]),
        Answer(id: "xp", question: "How do I earn XP?",
               reply: "Every Gym run earns 50 XP, and checking another trainer's result earns 50 more. You're level 2, 93 XP to level 3.",
               offers: [.enterGym]),
        Answer(id: "doing", question: "What is Coder doing?",
               reply: "Coder is working through 10 practice tasks with Project map. When it's done, we compare its score with today's.",
               prepared: false, followUps: ["tool"]),
        Answer(id: "result", question: "Why did Coder do better with Project map?",
               reply: "With the map, Coder found the right files first on two tasks it missed before. That's the jump from 6 to 8.",
               prepared: false, offers: [.seeResult, .goToGym],
               resultLine: "6 of 10 → 8 of 10 with Project map. Better."),
        Answer(id: "fix", question: "Fix the failing login test in my repo",
               reply: "We'll dispatch Coder to fix the failing login test.",
               prepared: false, offers: [.runCoder]),
        Answer(id: "fix-nocomputer", question: "Fix the failing login test in my repo",
               reply: "We'll dispatch Coder to fix the failing login test. It needs a computer to work on your code.",
               prepared: false, offers: [.connectComputer]),
        Answer(id: "wallet", question: "Where's my wallet?",
               reply: "Your wallet is under Profile, in Advanced.",
               offers: [.screen("Open Wallet")]),
    ]
    static func answer(_ id: String) -> Answer { answers.first { $0.id == id } ?? answers[0] }

    /// What a typed (free text) message gets in the mockup: the model's
    /// "streamed" reply after a short opener.
    static let freeTextOpener = "Here's how that works."
    static let freeTextReply = "In the real app, the model's answer streams in here, drawn as Markdown. This mockup doesn't read what you typed; tap a suggestion to see a prepared answer and its offers."

    static let firstTimeChips = ["who", "can", "cost", "gym"]
    static let recentChats = ["Fix login…", "Why did Coder…"]
    static let trainingChips = ["doing", "tool"]

    struct ChatSummary: Identifiable {
        let id = UUID()
        let title: String
        let when: String
        let onComputer: Bool
    }

    static let previousChats: [ChatSummary] = [
        ChatSummary(title: "Why did Coder do better with Project map?", when: "2m", onComputer: false),
        ChatSummary(title: "Fix the login test · Studio Mac", when: "1h", onComputer: true),
        ChatSummary(title: "What does it cost?", when: "Mon", onComputer: false),
    ]

    /// SCR-19: Coder's streamed work on a computer.
    static let coderRequest = "Fix the login test."
    static let coderReplyLines = [
        "Found it: the test expects the old redirect after sign-in.",
        "The new flow sends you to /home instead of /dashboard, so the assertion fails.",
        "I updated the expectation and the fixture.",
    ]
    static let coderCommand = "$ cargo test login"
    static let coderQuestion = "Keep the old redirect?"
    static let delegatedTo = "OpenCode"

    // MARK: Cinematic (CIN-01)

    struct Shot: Identifiable {
        let id: String
        let seconds: Double
        let subtitle: String
        let art: ShotArt
    }

    enum ShotArt { case emblem, gridCrane, plaza, gym, lights, loot, coder, endCard }

    /// Narration, word for word from the spec's script.
    static let shots: [Shot] = [
        Shot(id: "S01", seconds: 5, subtitle: "Every day, people ask AI agents to write their code.", art: .emblem),
        Shot(id: "S02", seconds: 7, subtitle: "One agent, working alone, only gets so good.", art: .gridCrane),
        Shot(id: "S03", seconds: 8, subtitle: "So we're doing it together. This is the Verse, where agents and people meet.", art: .plaza),
        Shot(id: "S04", seconds: 10, subtitle: "This is the Gym. Here, you train your agent. You give it a new tool, and we measure if it helps.", art: .gym),
        Shot(id: "S05", seconds: 10, subtitle: "When a tool helps, every agent gets it. Many agents, trained by many people, combined into one: OpenAgents.", art: .lights),
        Shot(id: "S06", seconds: 7, subtitle: "Train well, and you earn XP, loot, rewards, and glory.", art: .loot),
        Shot(id: "S07", seconds: 6, subtitle: "This is Coder. It's yours to train.", art: .coder),
        Shot(id: "S08", seconds: 4, subtitle: "Let's go to the Gym.", art: .endCard),
    ]

    // MARK: Report a problem (SCR-13)

    static let reportCode = "4F2A"
    static let chatMessageCount = 12
}
