import SwiftUI

// MOCK DATA: every fake number, name, and line of copy in the mockup.
//
// Screens read their words from here, so a copy change is one edit.
// Words follow the spec's "Words on screen" rules: plain words only, no
// jargon on primary surfaces (build.sh check greps for the banned words).
// Spec: docs/product/2026-09-28-app-wireframe.md, revision 3.

enum MockData {

    // MARK: Switches for the designer

    /// Show the "later" parts of the main menu (bell, CODER, RANKINGS, and
    /// THE GYM IN THE VERSE rows). The spec's v1 cut hides them; the Screen
    /// index also has a "v1Cut" state.
    static let showLaterFeatures = true
    /// The owner's reference shows a balance pill on the player card.
    /// The spec cuts it from v1 (no money on the main menu), so it is off.
    static let showBalancePill = false
    /// Show Skip during the first play of the cinematic too. The spec says
    /// the first play plays through; this is on so reviewing is quick.
    static let cinematicSkipAlways = true
    /// 1.0 plays the cinematic at the spec's durations (about 57 s).
    static let cinematicTimeScale: Double = 0.5
    /// Show SCR-06 Level up the first time a result is added, even though
    /// the XP only arrives when another trainer checks it, so the whole
    /// FLOW-01 is visible.
    static let levelUpOnFirstAdd = true
    /// How long a fake test run takes, in seconds (the real one is ~5 min).
    static let fakeRunSeconds: Double = 12
    /// After you add a result, another trainer "checks" it this many
    /// seconds later and the XP arrives (FLOW-10, simulated).
    static let fakeCheckSeconds: Double = 8
    /// Delay before a fake chat reply starts, and time per streamed word.
    static let fakeReplyDelay: Double = 0.7
    static let fakeStreamWordInterval: Double = 0.035

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
    /// XP you earn when another trainer checks a result you added.
    static let xpWhenChecked = 50
    /// XP you earn for checking another trainer's result.
    static let xpForACheck = 50
    /// XP you earn when Coder adopts your tool.
    static let xpWhenAdopted = 200
    static let runsLeftToday = 3
    static let newRunsAt = "9:00"
    static let testingNow = 38
    static let appVersion = "v0.1.0 · Playtest"
    static let season = "PLAYTEST SEASON 1 · ENDS OCT 26"
    static let seasonLine = "Help make Coder better. Every test counts."
    static let firstRunGreeting = "Hi, we're OpenAgents. Let's see if a tool makes Coder better."
    static let runMinutes = 5

    // MARK: Coder and the starter tests

    /// Starter tests Coder passes without any new tool (SCR-02) ...
    static let starterPassedToday = 5
    /// ... and with the tools it uses now (SCR-01.E06, SCR-07).
    static let starterPassedNow = 7
    static let starterTests = 8
    static let starterHistory = [5, 6, 7]
    static let starterHistoryWeeks = ["Sep 14", "Sep 21", "Sep 28"]

    // MARK: Tools

    struct Tool: Identifiable, Hashable {
        let id: String
        let name: String
        let icon: String          // SF Symbol name
        let line: String          // one plain sentence
        let testSet: String       // TestSet id
        let testedBy: Int
        let status: String        // SCR-08: Helps / Testing / Doesn't help
        /// CARD-01.E08: the newest checked result, or nil for "Not tested yet".
        let latest: String?
        let runs: Int
        let checks: Int
    }

    static let tools: [Tool] = [
        Tool(id: "project-map", name: "Project map", icon: "map",
             line: "Shows Coder how the project is laid out before it starts.",
             testSet: "project-map", testedBy: 18, status: "Helps",
             latest: "5 of 8 → 7 of 8 tests · Better · checked by 3 trainers", runs: 41, checks: 6),
        Tool(id: "code-finder", name: "Code finder", icon: "magnifyingglass",
             line: "Finds the right lines of code.",
             testSet: "code-finder", testedBy: 9, status: "Testing",
             latest: "5 of 8 → 5 of 8 tests · No clear change · checked by 3 trainers", runs: 14, checks: 3),
        Tool(id: "test-reader", name: "Test reader", icon: "checkmark.seal",
             line: "Reads test failures for Coder.",
             testSet: "test-reader", testedBy: 5, status: "Testing",
             latest: nil, runs: 8, checks: 1),
    ]
    static var defaultTool: Tool { tools[0] }
    static func tool(_ id: String) -> Tool { tools.first { $0.id == id } ?? tools[0] }

    // MARK: Test sets (SCR-21, CARD-02, the SCR-05 test list)

    struct Test: Hashable {
        let name: String          // the task, in the words Coder gets
        let checked: String       // how it's checked, one plain line
        var stayOut = false       // a test where the tool should stay out of the way
    }

    struct TestSet: Identifiable, Hashable {
        let id: String
        let toolName: String
        let madeBy: String        // SCR-21.E04
        let tests: [Test]
    }

    static let stayOutNote = "The tool should stay out of the way."

    static let testSets: [TestSet] = [
        TestSet(id: "project-map", toolName: "Project map", madeBy: "Made by OpenAgents · checked by 3 trainers", tests: [
            Test(name: "Find where login is handled", checked: "Checked: Coder names the right file, and looked at the project map."),
            Test(name: "Add a test for the date parser", checked: "Checked: a new test file exists and passes."),
            Test(name: "Explain the build setup", checked: "Checked: the answer names the build file and its main steps."),
            Test(name: "Rename a function everywhere it's used", checked: "Checked: the project builds and the old name is gone."),
            Test(name: "Change the port in the settings file", checked: "Checked: only the settings file changed."),
            Test(name: "List the folders that hold the app's screens", checked: "Checked: every folder named exists and holds screens."),
            Test(name: "Fix the failing sign-up test", checked: "Checked: the test passes and no other test breaks."),
            Test(name: "Leave a one-line fix alone", checked: "Checked: Coder didn't use the tool.", stayOut: true),
        ]),
        TestSet(id: "code-finder", toolName: "Code finder", madeBy: "Made by OpenAgents · checked by 3 trainers", tests: [
            Test(name: "Find where passwords are checked", checked: "Checked: Coder names the right file and line."),
            Test(name: "Find every place that sends email", checked: "Checked: all three places are named."),
            Test(name: "Find the code that rounds prices", checked: "Checked: Coder names the right function."),
            Test(name: "Fix the typo in the sign-in error", checked: "Checked: the message reads right and nothing else changed."),
            Test(name: "Find where the app reads the clock", checked: "Checked: Coder names the right file and line."),
            Test(name: "Remove a helper nothing uses", checked: "Checked: the helper is gone and the project builds."),
            Test(name: "Find the code behind the Save button", checked: "Checked: Coder names the right function."),
            Test(name: "Answer a question about the README", checked: "Checked: Coder didn't use the tool.", stayOut: true),
        ]),
        TestSet(id: "test-reader", toolName: "Test reader", madeBy: "Made by OpenAgents · not checked yet", tests: [
            Test(name: "Fix the failing date test", checked: "Checked: the test passes and no other test breaks."),
            Test(name: "Explain why the sign-in test fails", checked: "Checked: the answer names the real cause."),
            Test(name: "Fix a test that fails only sometimes", checked: "Checked: it passes ten times in a row."),
            Test(name: "Fix the test that runs out of time", checked: "Checked: the test passes in under a minute."),
            Test(name: "Find which change broke the build test", checked: "Checked: Coder names the right change."),
            Test(name: "Fix two failing tests in one file", checked: "Checked: both pass and no other test breaks."),
            Test(name: "Explain a failure in plain words", checked: "Checked: the answer names the file and the reason."),
            Test(name: "Add a helper when every test passes", checked: "Checked: Coder didn't use the tool.", stayOut: true),
        ]),
        TestSet(id: "test-reader-2px", toolName: "Test reader", madeBy: "Made by Trainer 2PX · checked by 1 trainer", tests: [
            Test(name: "Read a failing test and fix the code", checked: "Checked: the test passes and no other test breaks."),
            Test(name: "Fix the test that expects the wrong date", checked: "Checked: the test passes with the right date."),
            Test(name: "Explain three failures from one run", checked: "Checked: each failure has its own reason."),
            Test(name: "Fix the failing test after an upgrade", checked: "Checked: the test passes and the upgrade stays."),
            Test(name: "Find the test that fails on a new computer", checked: "Checked: Coder names the right test and why."),
            Test(name: "Fix the upload test that fails only sometimes", checked: "Checked: it passes ten times in a row."),
            Test(name: "Point to the line that made a test fail", checked: "Checked: Coder names the right line."),
            Test(name: "Rename a file when no test fails", checked: "Checked: Coder didn't use the tool.", stayOut: true),
        ]),
        TestSet(id: "changelog", toolName: "Changelog helper", madeBy: "Made by you · a draft only you can see", tests: [
            Test(name: "Summarize a merged fix", checked: "Checked: one line, past tense, names the fix."),
            Test(name: "Write the entry for a new setting", checked: "Checked: the entry names the setting and what it does."),
            Test(name: "Group three small changes", checked: "Checked: one entry with all three changes."),
            Test(name: "Note a change that breaks old versions", checked: "Checked: the entry starts with \"Breaking:\"."),
            Test(name: "Leave an unrelated question alone", checked: "Checked: Coder didn't use the tool.", stayOut: true),
        ]),
    ]
    static func testSet(_ id: String) -> TestSet { testSets.first { $0.id == id } ?? testSets[0] }

    // MARK: The tool made in chat (FLOW-07)

    static let madeToolName = "Changelog helper"
    static let madeToolIdea = "Help me make a changelog tool"
    static let madeToolAnswer = "One line, past tense. It names the fix and links the change."
    /// CARD-02.E06: how tests are checked, one plain line.
    static let draftCheckLine = "Each test is checked on Coder's last message and the files it made."
    /// FLOW-07: what chat says after the first try.
    static let firstTryNote = "Test 3 looks too easy: Coder passed it without the tool. Change it?"
    static let firstTryFix = "Done. Test 3 now groups changes from three different people. Run the full test set when you're ready."

    // MARK: Results (CARD-04, SCR-05)

    /// One result: which test set, and which tests passed without and with
    /// the tool. "1" passed, "0" didn't, one character per test, so the
    /// counts on every card and screen can't disagree.
    struct Outcome: Hashable {
        let headline: String
        let toolName: String
        let testSet: String
        let without: String
        let with: String
        /// SCR-05.E05: why it matters.
        let why: String
        /// CARD-04.E03 / SCR-05.E04.
        let xpLine: String
        var isFirstTry = false
        var isCheck = false

        var total: Int { without.count }
        var withoutCount: Int { without.filter { $0 == "1" }.count }
        var withCount: Int { with.filter { $0 == "1" }.count }
        func passedWithout(_ i: Int) -> Bool { Array(without)[i] == "1" }
        func passedWith(_ i: Int) -> Bool { Array(with)[i] == "1" }
        /// SCR-05.E09 share text.
        var shareText: String { "Coder passed \(withCount) of \(total) tests with \(toolName) instead of \(withoutCount)" }
    }

    static let checkedXPLine = "+\(xpWhenChecked) XP when another trainer checks it"
    static let checkXPLine = "+\(xpForACheck) XP once our referee confirms your check"

    static let outcomes: [String: Outcome] = [
        "better": Outcome(headline: "Coder got better", toolName: "Project map", testSet: "project-map",
                          without: "10011011", with: "11111011",
                          why: "When other trainers confirm it, Coder can use Project map for everyone.",
                          xpLine: checkedXPLine),
        "noChange": Outcome(headline: "No clear change", toolName: "Code finder", testSet: "code-finder",
                            without: "11011010", with: "11101010",
                            why: "That's useful too. Now everyone knows this tool doesn't help on these tests.",
                            xpLine: checkedXPLine),
        "worse": Outcome(headline: "Coder did worse with this tool", toolName: "Test reader", testSet: "test-reader",
                         without: "10110110", with: "10100110",
                         why: "That's useful too. We won't give Coder this tool.",
                         xpLine: checkedXPLine),
        "firstTry": Outcome(headline: "First try", toolName: madeToolName, testSet: "changelog",
                            without: "00101", with: "11101",
                            why: "One run is a first look, not a result. Run the full test set to add it to the Gym.",
                            xpLine: "No XP for a first try. It's just a look.", isFirstTry: true),
        "madeBetter": Outcome(headline: "Coder got better", toolName: madeToolName, testSet: "changelog",
                              without: "00001", with: "11011",
                              why: "When other trainers confirm it, Coder can use your tool for everyone.",
                              xpLine: checkedXPLine),
        "confirmed": Outcome(headline: "You confirmed it", toolName: "Test reader", testSet: "test-reader-2px",
                             without: "10010110", with: "11011110",
                             why: "Checks keep the Gym honest. Trainer 2PX earns XP too.",
                             xpLine: checkXPLine, isCheck: true),
        "didntHold": Outcome(headline: "It didn't hold up", toolName: "Test reader", testSet: "test-reader-2px",
                             without: "10010110", with: "10110100",
                             why: "That's useful too. Now everyone knows this result didn't repeat.",
                             xpLine: checkXPLine, isCheck: true),
    ]
    static func outcome(_ key: String) -> Outcome { outcomes[key] ?? outcomes["better"]! }

    /// The run of a starter tool's test set ends in this result (CARD-03 → CARD-04).
    static func outcomeKey(forTool id: String) -> String {
        switch id {
        case "code-finder": "noChange"
        case "test-reader": "worse"
        default: "better"
        }
    }

    // MARK: A check waiting (CARD-06)

    static let checkTrainer = "Trainer 2PX"
    static let checkTool = "Test reader"
    static let checkTestSet = "test-reader-2px"
    static let checkClaimBefore = 4
    static let checkClaimAfter = 6

    // MARK: Gym news (CARD-05)

    struct NewsItem: Identifiable, Hashable {
        var id: String { text }
        let text: String
        let detail: String
        /// The answer (and its card) a tap opens; nil for a changelog line.
        let opens: String?
    }

    static let news: [NewsItem] = [
        NewsItem(text: "Coder now uses Project map for everyone.", detail: "Oct 3 · 6 checks", opens: "testProjectMap"),
        NewsItem(text: "Trainer 2PX made a test set for Test reader.", detail: "1 check so far", opens: "checkAResult"),
        NewsItem(text: "Code finder: no clear change on 8 tests.", detail: "3 checks", opens: "testCodeFinder"),
        NewsItem(text: "Build 21: tests in chat.", detail: "Our changelog", opens: nil),
    ]
    static let newsOffer = "Check Trainer 2PX's result"
    static let newsSource = "From the Gym's records and our changelog."
    static let newsEmpty = "Nothing new since you last asked."

    // MARK: Credit (CARD-07, SCR-11.E10)

    struct CreditRow: Identifiable, Hashable {
        var id: String { text }
        let done: Bool            // ✓ earned, … waiting
        let text: String
        let xp: Int?
    }

    struct CreditGroup: Identifiable, Hashable {
        var id: String { title }
        let title: String
        let rows: [CreditRow]
    }

    static let credit: [CreditGroup] = [
        CreditGroup(title: "Project map test set", rows: [
            CreditRow(done: true, text: "Checked by Trainer 2PX", xp: 25),
            CreditRow(done: true, text: "Checked by Trainer QA4", xp: 25),
            CreditRow(done: false, text: "Waiting for a check", xp: nil),
        ]),
        CreditGroup(title: "Changelog helper (your tool)", rows: [
            CreditRow(done: true, text: "Coder uses it now", xp: xpWhenAdopted),
        ]),
    ]
    static let creditHonesty = "XP can't be spent. It shows what you did, with your name on it."
    static let creditEmpty = "Nothing yet. When another trainer checks a result you added, you earn XP here."
    static let creditShareText = "I helped make Coder better on OpenAgents: Changelog helper is now one of Coder's tools."

    // MARK: Your results and what you made (SCR-11)

    struct ResultRow: Identifiable {
        var id: String { tool }
        let tool: String
        let outcome: String       // key into outcomes
        let verdict: String
    }

    static let yourResults: [ResultRow] = [
        ResultRow(tool: "Project map", outcome: "better", verdict: "Better"),
        ResultRow(tool: "Code finder", outcome: "noChange", verdict: "No change"),
    ]

    struct MadeRow: Identifiable {
        var id: String { text }
        let text: String
        let xp: Int
    }

    static let whatYouMade: [MadeRow] = [
        MadeRow(text: "Changelog helper · Coder uses it", xp: xpWhenAdopted),
        MadeRow(text: "Test reader tests · 2 checks", xp: 50),
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
    static let yourWeekXP = 190
    static let xpToPassNext = 12

    // MARK: Updates (SCR-12)

    struct Update: Identifiable {
        var id: String { text }
        let unread: Bool
        let text: String
        let button: String
        let opens: String         // answer id
    }

    static let updates: [Update] = [
        Update(unread: true, text: "Trainer 2PX confirmed your result. +\(xpWhenChecked) XP is yours.", button: "SEE IT", opens: "credit"),
        Update(unread: true, text: "Coder now uses your tool for everyone. +\(xpWhenAdopted) XP.", button: "SEE IT", opens: "credit"),
        Update(unread: false, text: "A check is waiting for you.", button: "CHECK", opens: "checkAResult"),
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
        /// Sends another answer's question (a chip or a scripted step).
        case ask(String)
        /// Puts "Change: " in the composer (CARD-02.E07, FLOW-07).
        case changeIt
        /// Opens SCR-05 for the player's latest result.
        case seeResult

        /// A tap sends a message or edits the draft (shown only on the newest reply).
        var sendsMessage: Bool {
            switch self {
            case .ask, .changeIt: true
            default: false
            }
        }
    }

    struct CommandCard: Hashable {
        let command: String
        let note: String
        let output: String
    }

    /// A card the app draws inside a reply (CARD-01 … CARD-07).
    enum Card: Hashable {
        case tool(String)            // a tool id
        case draft
        case result(String)          // an outcome key
        case news(empty: Bool)
        case check
        case credit(empty: Bool)
    }

    /// A reply the mockup can give. Chips and offers point at one by id,
    /// so nothing here guesses what a typed message means.
    struct Answer: Hashable {
        let id: String
        let question: String
        let reply: String
        /// A reviewed bank answer (shows at once, "Prepared answer").
        var prepared = true
        /// Shown first while the rest streams in (only the model's replies).
        var opener: String? = nil
        var offers: [Offer] = []
        var followUps: [String] = []   // Answer ids
        var command: CommandCard? = nil
        var card: Card? = nil
        /// The chip's icon when this answer is offered as a starter chip.
        var icon = "questionmark.circle"
    }

    static let answers: [Answer] = [
        // CHAT-1: kicking the tires (prepared answers).
        Answer(id: "who", question: "Who are you?",
               reply: "We're OpenAgents. We answer questions here, and when a job needs a computer, we send Coder, our AI that writes code, to do it.",
               followUps: ["can", "cost"]),
        Answer(id: "can", question: "What can you do here?",
               reply: "We answer questions, explain things, and help you plan and write. We also test tools on Coder, so you can see if a tool makes it better. When something needs a computer, we dispatch Coder to one of yours.",
               offers: [.runCoder, .screen("Open Wallet")],
               followUps: ["model", "cost"],
               command: CommandCard(command: "openagents computer list",
                                    note: "Reads only. Runs on this phone.",
                                    output: "Studio Mac     online   openagents\nLaptop         offline  —")),
        Answer(id: "cost", question: "What does it cost?",
               reply: "Chatting with us is free. Testing tools on Coder is free too: we pay for the runs, up to 3 a day.",
               followUps: ["gym", "xp"]),
        Answer(id: "model", question: "What model is this?",
               reply: "Replies come from Gemini 3.8 Flash through our AI Gateway. Common questions get a reviewed answer at once.",
               followUps: ["who"]),
        Answer(id: "gym", question: "What's the Gym?",
               reply: "The Gym is where you help make Coder better. You pick a tool, we run a set of tests with the tool and without it, and you see if Coder passes more.",
               followUps: ["xp"], card: .tool("project-map")),
        Answer(id: "coder", question: "What is Coder?",
               reply: "Coder is our AI that writes code. You help it get better by testing tools on it. When a tool is confirmed to help, everyone's Coder gets it.",
               offers: [.ask("testATool")], followUps: ["gym"]),
        Answer(id: "xp", question: "How do I earn XP?",
               reply: "You earn XP when another trainer checks a result you added (+\(xpWhenChecked)), when you check someone's result (+\(xpForACheck)), and when Coder adopts a tool you made (+\(xpWhenAdopted)). You're level 2, 93 XP to level 3.",
               offers: [.ask("checkAResult"), .ask("credit")]),

        // CHAT-2 to CHAT-5: test a tool (cards from the Gym's records).
        Answer(id: "testATool", question: "Test a tool",
               reply: "We'd try Project map. 18 trainers tested it; most saw Coder pass more tests.",
               prepared: false, card: .tool("project-map"), icon: "dumbbell"),
        Answer(id: "whichTool", question: "Which tool should I try?",
               reply: "Try Project map. 18 trainers tested it; most saw Coder pass more tests.",
               prepared: false, card: .tool("project-map")),
        Answer(id: "next", question: "What should I try next?",
               reply: "Try Project map. 18 trainers tested it; most saw Coder pass more tests.",
               prepared: false, card: .tool("project-map")),
        Answer(id: "testProjectMap", question: "Test Project map on Coder",
               reply: "We'll run 8 tests with Project map and without it. It takes about 5 minutes.",
               prepared: false, card: .tool("project-map"), icon: "map"),
        Answer(id: "testCodeFinder", question: "Test Code finder on Coder",
               reply: "We'll run 8 tests with Code finder and without it. It takes about 5 minutes.",
               prepared: false, card: .tool("code-finder"), icon: "magnifyingglass"),
        Answer(id: "testTestReader", question: "Test Test reader on Coder",
               reply: "We'll run 8 tests with Test reader and without it. It takes about 5 minutes.",
               prepared: false, card: .tool("test-reader"), icon: "checkmark.seal"),
        Answer(id: "tool", question: "What does Project map do?",
               reply: "Project map shows Coder how the project is laid out before it starts, so it spends less time looking around. It only reads the project; it can't change files or go online.",
               prepared: false, card: .tool("project-map")),
        Answer(id: "doing", question: "What is Coder doing?",
               reply: "Coder is working through 8 tests with Project map and the same 8 without it. When it's done, we show how many it passed each way.",
               prepared: false, followUps: ["tool"]),
        Answer(id: "howDid", question: "How did my test do?",
               reply: "Here's your latest result.",
               prepared: false, card: .result("better")),
        Answer(id: "result", question: "Why did Coder do better with Project map?",
               reply: "With the map, Coder found the right files first on two tests it missed without it: adding a test for the date parser, and explaining the build setup. That's the jump from 5 to 7.",
               prepared: false, offers: [.seeResult, .ask("testATool")]),

        // CHAT-9: what's new. CHAT-13: check a result. CHAT-14: credit.
        Answer(id: "whatsNew", question: "What's new?",
               reply: "Here's what's new in the Gym this week.",
               prepared: false, card: .news(empty: false), icon: "newspaper"),
        Answer(id: "whatsNewEmpty", question: "What's new in the Gym?",
               reply: "Here's what we found.",
               prepared: false, card: .news(empty: true), icon: "newspaper"),
        Answer(id: "checkAResult", question: "Check a result",
               reply: "Trainer 2PX added a result that needs a check. Run the same tests to see if it holds up.",
               prepared: false, card: .check, icon: "checkmark.circle"),
        Answer(id: "anyChecks", question: "Is there a result I can check?",
               reply: "Yes. Trainer 2PX added a result that needs a check.",
               prepared: false, card: .check, icon: "checkmark.circle"),
        Answer(id: "credit", question: "What have I earned?",
               reply: "Here's what your work has earned so far.",
               prepared: false, card: .credit(empty: false), icon: "star"),
        Answer(id: "creditEmpty", question: "Did anyone check my tests?",
               reply: "Not yet.",
               prepared: false, card: .credit(empty: true), icon: "star"),

        // CHAT-10: make a tool and its tests (FLOW-07, the scripted interview).
        Answer(id: "makeTool", question: madeToolIdea,
               reply: "Happy to. First: when Coder writes a changelog entry, what does a good one look like?",
               prepared: false, offers: [.ask("makeToolAnswer")], icon: "hammer"),
        Answer(id: "makeToolAnswer", question: madeToolAnswer,
               reply: "Here's what we'd make: a tool that tells Coder how you write entries (a short guide Coder follows), plus Project map to find the change.",
               prepared: false, offers: [.ask("makeToolLooksGood"), .changeIt], icon: "text.bubble"),
        Answer(id: "makeToolLooksGood", question: "Looks good",
               reply: "Here are 5 tests for it. The last one checks that the tool stays out of the way when it isn't needed.",
               prepared: false, card: .draft, icon: "checkmark"),
        Answer(id: "makeTest3Harder", question: "Make test 3 harder",
               reply: firstTryFix, prepared: false, icon: "pencil"),

        // CHAT-7: work on your own code needs a computer.
        Answer(id: "fix", question: "Fix the failing login test in my repo",
               reply: "Working on fixing the failing login test.",
               prepared: false, offers: [.runCoder]),
        Answer(id: "fix-nocomputer", question: "Fix the failing login test in my repo",
               reply: "Working on fixing the failing login test. It needs a computer to work on your code.",
               prepared: false, offers: [.connectComputer]),
        // CHAT-8: support.
        Answer(id: "wallet", question: "Where's my wallet?",
               reply: "Your wallet is under Profile, in Advanced.",
               offers: [.screen("Open Wallet")]),
    ]
    static func answer(_ id: String) -> Answer { answers.first { $0.id == id } ?? answers[0] }

    /// What a typed (free text) message gets in the mockup: the model's
    /// "streamed" reply after a short opener.
    static let freeTextOpener = "We'll look that up for you."
    static let freeTextReply = "In the real app, our answer streams in here, drawn as Markdown. This preview doesn't read what you typed; tap a suggestion to see a prepared answer, a card, and its offers."

    /// SCR-15 chips: first-time questions, and the starter chips (SCR-01.E13).
    static let firstTimeChips = ["who", "can", "cost", "gym"]
    static let starterChips = ["testATool", "whatsNew", "checkAResult"]
    static let recentChats = ["Fix login…", "Why did Coder…"]

    struct ChatSummary: Identifiable {
        var id: String { title }
        let title: String
        let when: String
        let onComputer: Bool
        let opens: String         // answer id, or "coder" for SCR-19
    }

    static let previousChats: [ChatSummary] = [
        ChatSummary(title: "Why did Coder do better with Project map?", when: "2m", onComputer: false, opens: "result"),
        ChatSummary(title: "Help me make a changelog tool", when: "40m", onComputer: false, opens: "makeTool"),
        ChatSummary(title: "Fix the login test · Studio Mac", when: "1h", onComputer: true, opens: "coder"),
        ChatSummary(title: "What does it cost?", when: "Mon", onComputer: false, opens: "cost"),
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
        Shot(id: "S04", seconds: 10, subtitle: "This is the Gym. Here, you train your agent. You give it a new tool, and we test if it helps.", art: .gym),
        Shot(id: "S05", seconds: 10, subtitle: "When a tool helps, every agent gets it. Many agents, trained by many people, combined into one: OpenAgents.", art: .lights),
        Shot(id: "S06", seconds: 7, subtitle: "Train well, and you earn XP, loot, rewards, and glory.", art: .loot),
        Shot(id: "S07", seconds: 6, subtitle: "This is Coder. It's yours to train.", art: .coder),
        Shot(id: "S08", seconds: 4, subtitle: "Let's see if a tool makes Coder better.", art: .endCard),
    ]

    // MARK: Report a problem (SCR-13)

    static let reportCode = "4F2A"
    static let chatMessageCount = 12
}
