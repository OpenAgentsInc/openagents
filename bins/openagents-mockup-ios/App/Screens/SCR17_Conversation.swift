import SwiftUI

// SCR-17 Chat: a conversation with OpenAgents, where the loop happens.
// Replies are fake: a prepared answer shows whole after a short wait; any
// other reply streams word by word. A reply can carry a chat card
// (CARD-01 … CARD-07, drawn by Screens/Cards/); a card's button does the
// work, and a newer card of the same thing replaces the older one in place
// (CARD-01 → CARD-03 → CARD-04). Offers act only on a tap (CHK-11).
// The same view is the first-run chat, SCR-15.E12 (state .firstRun).

enum SCR17State: Hashable {
    /// A conversation that starts with one of MockData.answers.
    case answer(String)
    /// A typed message; gets the generic streamed reply.
    case freeText(String)
    case thinking
    case failed
    case afterRunCoder
    case noComputer
    /// SCR-18 inline states under a prepared answer.
    case wrongAnswer(SCR18State)
    /// SCR-15.E12: FLOW-01 step 2 of 3, the greeting and CARD-01.
    case firstRun
    /// One card in one state (the Screen index's CARD-nn entries).
    case card(ChatCard)
    /// SCR-01.E12 while a run is going: the chat with its CARD-03.
    case resumeRun
}

struct ChatMessage: Identifiable {
    enum Phase { case thinking, streaming, done, failed }
    let id = UUID()
    let mine: Bool
    var text: String
    var answer: MockData.Answer? = nil
    var phase: Phase = .done
    var ranCoder = false
    var wrongAnswerStart: SCR18State = .idle
    var card: ChatCard? = nil
}

struct SCR17Conversation: View {
    @Environment(MockApp.self) private var app
    let state: SCR17State
    @State private var messages: [ChatMessage] = []
    @State private var text = ""
    @State private var started = false
    @State private var chatID = UUID()
    /// FLOW-01: step 2 of 3 until the run starts, then 3.
    @State private var step = 2
    @State private var scrollTarget: UUID?
    @FocusState private var focused: Bool

    private var isFirstRun: Bool { state == .firstRun }

    var body: some View {
        VStack(spacing: 0) {
            header

            ScrollViewReader { proxy in
                ScrollView {
                    VStack(alignment: .leading, spacing: Theme.Space.m) {
                        ForEach($messages) { $m in
                            messageView($m).id(m.id)
                        }
                    }
                    .padding(.horizontal, Theme.Space.page)
                    .padding(.vertical, Theme.Space.m)
                }
                .scrollDismissesKeyboard(.interactively)
                .onTapGesture { focused = false }
                .onChange(of: messages.last?.text) { scroll(proxy, to: messages.last?.id) }
                .onChange(of: scrollTarget) { scroll(proxy, to: scrollTarget) }
            }

            // E10
            Composer(text: $text, focused: $focused) { sendTyped() }
                .padding(.horizontal, Theme.Space.page)
                .padding(.bottom, Theme.Space.xs)
                .padding(.top, 6)
        }
        .foregroundStyle(Theme.Colors.textPrimary)
        .background(Theme.Colors.background.ignoresSafeArea())
        .task { await seed() }
        .onAppear {
            app.activeChat = chatID
            drain()
        }
        .onChange(of: app.inbox) { drain() }
    }

    // MARK: Header

    @ViewBuilder private var header: some View {
        if isFirstRun {
            // SCR-15.E12: the step, no < Menu, no ☰ (the path can't be left half-done).
            ZStack {
                Text("OpenAgents").font(.system(size: 19, weight: .bold))
                HStack {
                    Text("STEP \(step) OF 3")
                        .condensedTitle(Theme.Fonts.sectionLabel, tracking: Theme.Tracking.sectionLabel)
                        .foregroundStyle(Theme.Colors.textSecondary)
                        .onLongPressGesture { app.showIndex = true }
                    Spacer()
                    StepDots(step: step)
                }
            }
            .padding(.horizontal, Theme.Space.page)
            .frame(height: Theme.Size.topBarHeight)
            .overlay(alignment: .bottom) { Rectangle().fill(Theme.Colors.divider).frame(height: 1) }
        } else {
            // E01
            ChatHeader(back: BackControl(label: "") { app.back() },
                       onMenu: { app.go(.previousChats(.normal)) },
                       title: "OpenAgents") {
                Button { app.go(.newChat(.returning)) } label: {
                    Image(systemName: "square.and.pencil").font(.system(size: 19, weight: .semibold))
                        .frame(width: 44, height: 44)
                }
            }
        }
    }

    // MARK: Messages

    @ViewBuilder
    private func messageView(_ m: Binding<ChatMessage>) -> some View {
        let msg = m.wrappedValue
        if msg.mine {
            // E02
            Bubble(text: msg.text, mine: true)
        } else {
            VStack(alignment: .leading, spacing: Theme.Space.s) {
                switch msg.phase {
                case .thinking:
                    ThinkingLabel()
                case .failed:
                    // E13
                    Text("We couldn't answer that. Check your connection.")
                        .font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                    Chip(icon: "arrow.clockwise", text: "Try again") { retry(m) }
                case .streaming, .done:
                    // E03
                    if !msg.text.isEmpty { Bubble(text: msg.text, mine: false) }
                    if msg.phase == .done {
                        // E11: the card under the reply that introduced it.
                        if msg.card != nil { cardView(m) }
                        if let a = msg.answer {
                            // E04
                            if a.prepared {
                                Text("Prepared answer").font(Theme.Fonts.caption).foregroundStyle(Theme.Colors.textTertiary)
                            }
                            // E05, E06, E11. Offers that send a message (and
                            // follow-ups) show only under the newest reply, so an
                            // interview step can't be tapped twice.
                            let offers = a.offers.filter { isNewestReply(msg.id) || !$0.sendsMessage }
                            if !offers.isEmpty {
                                FlowLayout {
                                    ForEach(Array(offers.enumerated()), id: \.offset) { i, offer in
                                        offerChip(offer, primary: i == 0 && msg.card == nil, message: m)
                                    }
                                }
                            }
                            // E07
                            if let card = a.command { CommandCardView(card: card) }
                            // E08
                            if !a.followUps.isEmpty && isNewestReply(msg.id) {
                                FlowLayout {
                                    ForEach(a.followUps, id: \.self) { id in
                                        Chip(icon: "questionmark.circle", text: MockData.answer(id).question) { ask(id) }
                                    }
                                }
                            }
                            // E09 → SCR-18
                            if a.prepared {
                                SCR18WrongAnswer(start: msg.wrongAnswerStart)
                            }
                        }
                    }
                }
            }
        }
    }

    private func isNewestReply(_ id: UUID) -> Bool {
        messages.last(where: { !$0.mine })?.id == id
    }

    // MARK: Cards (E11)

    @ViewBuilder
    private func cardView(_ m: Binding<ChatMessage>) -> some View {
        let id = m.wrappedValue.id
        switch m.wrappedValue.card {
        case .tool(let toolID, let s):
            CARD01Tool(toolID: toolID, state: s, firstRun: isFirstRun,
                       onStart: { startRun(.tool(toolID), in: id) },
                       onRetry: { setCard(id, .tool(toolID, app.runsLeft > 0 ? .ready : .noRunsLeft)) },
                       onSeeTests: { app.present(.testSet(MockData.tool(toolID).testSet, draft: false)) },
                       onAsk: { ask($0) })
        case .draft(let s):
            CARD02Draft(step: s,
                        onLooksGood: { advanceDraft(id) },
                        onTryOnce: { startRun(.tryOnce, in: id) },
                        onChangeIt: { changeIt() },
                        onSeeEvery: { app.present(.testSet("changelog", draft: s != .ready)) })
        case .run(let kind, let s, let startedAt):
            CARD03Run(kind: kind, state: s, startedAt: startedAt, firstRun: isFirstRun,
                      onDone: { finishRun(kind, in: id) },
                      onStop: { stopRun(kind, in: id) },
                      onRetry: { setCard(id, .run(kind, .running, Date())) })
        case .result(let key, let added):
            CARD04Result(outcomeKey: key, added: added,
                         onAdd: { app.present(.addToGym(.normal, key, id)) },
                         onRunFull: { startRun(.fullDraft, in: id) },
                         onDetails: { app.go(.result(SCR05State(outcomeKey: key, added: added), id)) },
                         onSeeTests: { app.present(.testSet(MockData.outcome(key).testSet, draft: false)) })
        case .news(let s):
            CARD05News(state: s, onAsk: { ask($0) })
        case .check(let s):
            CARD06Check(state: s,
                        onRun: { startRun(.check, in: id) },
                        onSeeTests: { app.present(.testSet(MockData.checkTestSet, draft: false)) },
                        onAsk: { ask($0) })
        case .credit(let s):
            CARD07Credit(state: s, onAsk: { ask($0) })
        case nil:
            EmptyView()
        }
    }

    private func setCard(_ id: UUID, _ card: ChatCard) {
        guard let i = messages.firstIndex(where: { $0.id == id }) else { return }
        withAnimation(Theme.Motion.screen) { messages[i].card = card }
        scrollTarget = id
    }

    /// START THE TEST, RUN THE CHECK, TRY IT ONCE, RUN THE FULL TEST SET:
    /// the card becomes CARD-03.
    private func startRun(_ kind: RunKind, in id: UUID) {
        let now = Date()
        app.startRun(tool: kind.toolName, spendsRun: kind.spendsRun)
        app.runStartedAt = now
        if isFirstRun {
            step = 3
            app.firstRunStep = .running
        }
        setCard(id, .run(kind, .running, now))
    }

    /// The run finished: CARD-03 becomes CARD-04.
    private func finishRun(_ kind: RunKind, in id: UUID) {
        app.stopRun(refund: false)
        if isFirstRun { app.firstRunStep = .result }
        setCard(id, .result(kind.outcomeKey, added: false))
        if kind == .tryOnce {
            // FLOW-07: what we noticed, and the fix as a tap.
            var a = MockData.Answer(id: "firstTryNote", question: "", reply: MockData.firstTryNote, prepared: false)
            a.offers = [.ask("makeTest3Harder")]
            messages.append(ChatMessage(mine: false, text: a.reply, answer: a))
        }
    }

    /// Stop: back to the card that started it; the run isn't used.
    private func stopRun(_ kind: RunKind, in id: UUID) {
        app.stopRun(refund: kind.spendsRun)
        if isFirstRun {
            step = 2
            app.firstRunStep = .chat
        }
        switch kind {
        case .tool(let toolID): setCard(id, .tool(toolID, .ready))
        case .check: setCard(id, .check(.ready))
        case .tryOnce: setCard(id, .draft(.ready))
        case .fullDraft: setCard(id, .result("firstTry", added: false))
        }
    }

    /// CARD-02 LOOKS GOOD: the tests, then the checks, then TRY IT ONCE.
    private func advanceDraft(_ id: UUID) {
        guard let i = messages.firstIndex(where: { $0.id == id }), case .draft(let s) = messages[i].card else { return }
        switch s {
        case .tests: setCard(id, .draft(.checks))
        case .checks, .ready: setCard(id, .draft(.ready))
        }
    }

    /// CARD-02.E07 and the interview's Change it: the cursor in the composer.
    private func changeIt() {
        text = "Change: "
        focused = true
    }

    @ViewBuilder
    private func offerChip(_ offer: MockData.Offer, primary: Bool, message: Binding<ChatMessage>) -> some View {
        switch offer {
        case .runCoder:
            if message.wrappedValue.ranCoder {
                Chip(icon: "desktopcomputer", text: "Open Coder on \(MockData.computerName)", filled: primary) {
                    app.go(.coderChat(.working))
                }
            } else {
                Chip(icon: "desktopcomputer", text: "Run Coder on \(MockData.computerName)", filled: primary) {
                    message.wrappedValue.ranCoder = true
                    app.go(.coderChat(.working))
                }
            }
        case .openCoder:
            Chip(icon: "desktopcomputer", text: "Open Coder on \(MockData.computerName)", filled: primary) {
                app.go(.coderChat(.working))
            }
        case .connectComputer:
            Chip(icon: "plus", text: "Connect a computer", filled: primary) { app.go(.stub("Your computers")) }
        case .screen(let name):
            Chip(icon: screenIcon(name), text: name, filled: primary) {
                if name == "Report a problem" { app.present(.report(.formFromChat)) }
                else { app.go(.stub(name.replacingOccurrences(of: "Open ", with: ""))) }
            }
        case .ask(let id):
            let a = MockData.answer(id)
            Chip(icon: a.icon, text: a.question, filled: primary) { ask(id) }
        case .changeIt:
            Chip(icon: "pencil", text: "Change it", filled: primary) { changeIt() }
        case .seeResult:
            Chip(icon: "chart.bar.fill", text: "See the result", filled: primary) { app.go(.result(.better, nil)) }
        }
    }

    private func screenIcon(_ name: String) -> String {
        switch name {
        case "Open Wallet": "wallet.pass"
        case "Your computers": "desktopcomputer"
        case "Identity keys": "key"
        case "Playtest": "gamecontroller"
        default: "exclamationmark.bubble"
        }
    }

    // MARK: The mailbox (SCR-05, SCR-20, SCR-21, and the simulated check)

    private func drain() {
        guard app.activeChat == chatID, !app.inbox.isEmpty else { return }
        for command in app.take() { handle(command) }
    }

    private func handle(_ command: ChatCommand) {
        switch command {
        case .added(let id):
            guard let i = messages.firstIndex(where: { $0.id == id }),
                  case .result(let key, _) = messages[i].card else { return }
            setCard(id, .result(key, added: true))
            let check = MockData.outcome(key).isCheck
            messages.append(ChatMessage(mine: false, text: check
                ? "Added. +\(MockData.xpForACheck) XP once our referee confirms it. \(MockData.checkTrainer) earns XP too."
                : "Added to the Gym. You'll earn XP when another trainer checks it."))
        case .runFullTestSet(let id):
            let target = id ?? messages.last(where: { if case .result("firstTry", _) = $0.card { true } else { false } })?.id
            if let target { startRun(.fullDraft, in: target) }
        case .approveDraft:
            if let m = messages.last(where: { if case .draft = $0.card { true } else { false } }) { advanceDraft(m.id) }
        case .send(let id):
            ask(id)
        case .checkedByOther:
            // FLOW-10: only in the chat where a result was added.
            guard messages.contains(where: { if case .result(_, true) = $0.card { true } else { false } }) else { return }
            messages.append(ChatMessage(mine: false,
                                        text: "\(MockData.checkTrainer) checked your result, and it held up. +\(MockData.xpWhenChecked) XP is yours.",
                                        card: .credit(.rows)))
            app.checkNotice = false
        }
    }

    // MARK: Fake replies

    private func seed() async {
        guard !started else { return }
        started = true
        switch state {
        case .answer(let id): await ask(id)
        case .freeText(let t): await say(t)
        case .noComputer: await ask("fix-nocomputer")
        case .afterRunCoder:
            var a = MockData.answer("fix")
            a.offers = [.runCoder]
            messages = [ChatMessage(mine: true, text: a.question),
                        ChatMessage(mine: false, text: a.reply, answer: a, ranCoder: true)]
        case .thinking:
            messages = [ChatMessage(mine: true, text: "What can you do here?"),
                        ChatMessage(mine: false, text: "", phase: .thinking)]
        case .failed:
            messages = [ChatMessage(mine: true, text: "What can you do here?"),
                        ChatMessage(mine: false, text: "", phase: .failed)]
        case .wrongAnswer(let s):
            let a = MockData.answer("can")
            messages = [ChatMessage(mine: true, text: a.question),
                        ChatMessage(mine: false, text: a.reply, answer: a, wrongAnswerStart: s)]
        case .firstRun:
            // FLOW-01 rule 1: reopen at the furthest step (the run, or its result).
            let card: ChatCard
            switch app.firstRunStep {
            case .running:
                card = .run(.tool("project-map"), .running, app.runStartedAt ?? Date())
                step = 3
            case .result:
                card = .result("better", added: false)
                step = 3
            default:
                card = .tool("project-map", .ready)
                if app.firstRunStep != .done { app.firstRunStep = .chat }
            }
            messages = [ChatMessage(mine: false, text: MockData.firstRunGreeting, card: card)]
        case .card(let card):
            let intro = Self.intro(for: card)
            messages = [ChatMessage(mine: true, text: intro.question),
                        ChatMessage(mine: false, text: intro.reply, card: card)]
        case .resumeRun:
            let a = MockData.answer("testATool")
            messages = [ChatMessage(mine: true, text: a.question),
                        ChatMessage(mine: false, text: a.reply,
                                    card: .run(.tool("project-map"), .running, app.runStartedAt ?? Date()))]
        }
    }

    /// What was asked and answered above a card opened on its own.
    static func intro(for card: ChatCard) -> (question: String, reply: String) {
        func from(_ id: String) -> (String, String) { let a = MockData.answer(id); return (a.question, a.reply) }
        switch card {
        case .tool(let id, _):
            switch id {
            case "code-finder": return from("testCodeFinder")
            case "test-reader": return from("testTestReader")
            default: return from("testATool")
            }
        case .draft: return from("makeToolLooksGood")
        case .run(let kind, _, _):
            switch kind {
            case .check: return ("Check a result", "Running the same tests Trainer 2PX ran.")
            case .tryOnce: return ("Try it once", "Trying your tests once, with and without the tool.")
            case .fullDraft: return ("Run the full test set", "Running all 5 tests, three times each way.")
            case .tool: return ("Test a tool", "Started. We'll post the result here.")
            }
        case .result(let key, _):
            switch key {
            case "firstTry": return ("Try it once", "Here's how the first try went.")
            case "confirmed", "didntHold": return ("Run the check", "Here's what your check found.")
            case "madeBetter": return ("Run the full test set", "Here's the full result for your tool.")
            default: return from("howDid")
            }
        case .news(let s): return from(s == .empty ? "whatsNewEmpty" : "whatsNew")
        case .check(let s): return s == .noneWaiting ? ("Check a result", "Not right now.") : from("checkAResult")
        case .credit(let s): return from(s == .empty ? "creditEmpty" : "credit")
        }
    }

    @MainActor
    private func ask(_ id: String) async {
        let a = MockData.answer(id)
        messages.append(ChatMessage(mine: true, text: a.question))
        await reply(a)
    }

    private func ask(_ id: String) { Task { await ask(id) } }

    @MainActor
    private func say(_ t: String) async {
        messages.append(ChatMessage(mine: true, text: t))
        await reply(MockData.Answer(id: "free", question: t, reply: MockData.freeTextReply,
                                    prepared: false, opener: MockData.freeTextOpener))
    }

    @MainActor
    private func reply(_ a: MockData.Answer) async {
        messages.append(ChatMessage(mine: false, text: "", phase: .thinking))
        let i = messages.count - 1
        try? await Task.sleep(for: .seconds(MockData.fakeReplyDelay))
        let card = a.card.map { ChatCard($0, runsLeft: app.runsLeft) }
        if a.prepared {
            messages[i].text = a.reply
            messages[i].answer = a
            messages[i].card = card
            withAnimation { messages[i].phase = .done }
            return
        }
        // The model's reply streams in (after a short opener, if any).
        messages[i].phase = .streaming
        var shown = ""
        if let opener = a.opener {
            messages[i].text = opener
            try? await Task.sleep(for: .seconds(0.5))
            shown = opener + " "
        }
        for word in a.reply.split(separator: " ") {
            shown += word + " "
            messages[i].text = shown
            try? await Task.sleep(for: .seconds(MockData.fakeStreamWordInterval))
        }
        messages[i].text = shown.trimmingCharacters(in: .whitespaces)
        messages[i].answer = a
        messages[i].card = card
        withAnimation { messages[i].phase = .done }
        scrollTarget = messages[i].id
    }

    private func retry(_ m: Binding<ChatMessage>) {
        m.wrappedValue.phase = .thinking
        Task {
            try? await Task.sleep(for: .seconds(MockData.fakeReplyDelay))
            let a = MockData.answer("can")
            m.wrappedValue.text = a.reply
            m.wrappedValue.answer = a
            withAnimation { m.wrappedValue.phase = .done }
        }
    }

    private func sendTyped() {
        let t = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !t.isEmpty else { return }
        text = ""
        Task { await say(t) }
    }

    private func scroll(_ proxy: ScrollViewProxy, to id: UUID?) {
        guard let id else { return }
        withAnimation { proxy.scrollTo(id, anchor: .bottom) }
    }
}

/// "Thinking" with three pulsing dots (spec: not "Coder is thinking").
struct ThinkingLabel: View {
    @State private var on = false

    var body: some View {
        HStack(spacing: 6) {
            Text("Thinking").font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
            ForEach(0..<3, id: \.self) { i in
                Circle().fill(Theme.Colors.textSecondary).frame(width: 5, height: 5)
                    .opacity(on ? 1 : 0.2)
                    .animation(.easeInOut(duration: 0.6).repeatForever().delay(Double(i) * 0.2), value: on)
            }
        }
        .onAppear { on = true }
    }
}

#Preview("SCR-17 Conversation") {
    NavigationStack { SCR17Conversation(state: .answer("can")) }.environment(MockApp())
}

#Preview("SCR-17 Test a tool") {
    NavigationStack { SCR17Conversation(state: .answer("testATool")) }.environment(MockApp())
}

#Preview("SCR-17 Make a tool") {
    NavigationStack { SCR17Conversation(state: .answer("makeTool")) }.environment(MockApp())
}

#Preview("SCR-17 Result card") {
    NavigationStack { SCR17Conversation(state: .card(.result("better", added: false))) }.environment(MockApp())
}

#Preview("SCR-17 Streaming") {
    NavigationStack { SCR17Conversation(state: .freeText("How does the Gym measure a tool?")) }.environment(MockApp())
}

#Preview("SCR-17 Failed") {
    NavigationStack { SCR17Conversation(state: .failed) }.environment(MockApp())
}

#Preview("SCR-15.E12 First-run chat") {
    NavigationStack { SCR17Conversation(state: .firstRun) }.environment(MockApp())
}
