import SwiftUI

// SCR-17 Chat: a conversation with OpenAgents. Replies are fake: a
// prepared answer shows whole after a short wait; any other reply shows an
// opener, then streams word by word. Offers act only on a tap (CHK-11).

enum SCR17State: Hashable {
    /// A conversation that starts with one of MockData.answers.
    case answer(String)
    /// A typed message; gets the generic streamed reply.
    case freeText(String)
    case thinking
    case failed
    case afterRunCoder
    case noComputer
    case aboutResult
    case aboutTool
    /// SCR-18 inline states under a prepared answer.
    case wrongAnswer(SCR18State)
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
}

struct SCR17Conversation: View {
    @Environment(MockApp.self) private var app
    let state: SCR17State
    @State private var messages: [ChatMessage] = []
    @State private var text = ""
    @State private var started = false
    @FocusState private var focused: Bool

    var body: some View {
        VStack(spacing: 0) {
            // E01
            ChatHeader(back: BackControl(label: "") { app.back() },
                       onMenu: { app.go(.previousChats(.normal)) },
                       title: "OpenAgents") {
                Button { app.go(.newChat(.returning)) } label: {
                    Image(systemName: "square.and.pencil").font(.system(size: 19, weight: .semibold))
                        .frame(width: 44, height: 44)
                }
            }

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
                .onChange(of: messages.last?.text) {
                    if let id = messages.last?.id { withAnimation { proxy.scrollTo(id, anchor: .bottom) } }
                }
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
                    Bubble(text: msg.text, mine: false)
                    if let a = msg.answer, msg.phase == .done {
                        // E12
                        if let line = a.resultLine {
                            Text(line).font(Theme.Fonts.bodyBold)
                                .padding(10)
                                .background(RoundedRectangle(cornerRadius: 10).fill(Theme.Colors.surfaceRaised))
                        }
                        // E04
                        if a.prepared {
                            Text("Prepared answer").font(Theme.Fonts.caption).foregroundStyle(Theme.Colors.textTertiary)
                        }
                        // E05, E06, E11
                        if !a.offers.isEmpty {
                            FlowLayout {
                                ForEach(Array(a.offers.enumerated()), id: \.offset) { i, offer in
                                    offerChip(offer, primary: i == 0, message: m)
                                }
                            }
                        }
                        // E07
                        if let card = a.command { CommandCardView(card: card) }
                        // E08
                        if !a.followUps.isEmpty {
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
        case .goToGym:
            Chip(icon: "dumbbell.fill", text: "Go to the Gym", filled: primary) { app.go(.gym(.returning)) }
        case .trainWithTool:
            Chip(icon: "map", text: "Train with this tool", filled: primary) {
                app.selectedToolID = MockData.defaultTool.id
                app.go(.gym(.returning))
            }
        case .startTraining:
            Chip(icon: "play.fill", text: "Start training", filled: primary) { app.go(.gym(.returning)) }
        case .seeResult:
            Chip(icon: "chart.bar.fill", text: "See the result", filled: primary) { app.go(.result(.better)) }
        case .enterGym:
            Chip(icon: "dumbbell.fill", text: "Enter the Gym", filled: primary) { app.go(.gym(.returning)) }
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

    // MARK: Fake replies

    private func seed() async {
        guard !started else { return }
        started = true
        switch state {
        case .answer(let id): await ask(id)
        case .freeText(let t): await say(t)
        case .aboutResult: await ask("result")
        case .aboutTool: await ask("tool")
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
        await reply(MockData.Answer(id: "free", question: t, reply: MockData.freeTextReply, prepared: false))
    }

    @MainActor
    private func reply(_ a: MockData.Answer) async {
        messages.append(ChatMessage(mine: false, text: "", phase: .thinking))
        let i = messages.count - 1
        try? await Task.sleep(for: .seconds(MockData.fakeReplyDelay))
        if a.prepared {
            messages[i].text = a.reply
            messages[i].answer = a
            withAnimation { messages[i].phase = .done }
            return
        }
        // Opener, then the "model" streams in.
        messages[i].phase = .streaming
        messages[i].text = MockData.freeTextOpener
        try? await Task.sleep(for: .seconds(0.5))
        var shown = MockData.freeTextOpener + " "
        for word in a.reply.split(separator: " ") {
            shown += word + " "
            messages[i].text = shown
            try? await Task.sleep(for: .seconds(MockData.fakeStreamWordInterval))
        }
        messages[i].text = shown.trimmingCharacters(in: .whitespaces)
        messages[i].answer = a
        withAnimation { messages[i].phase = .done }
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

#Preview("SCR-17 Streaming") {
    NavigationStack { SCR17Conversation(state: .freeText("How does the Gym measure a tool?")) }.environment(MockApp())
}

#Preview("SCR-17 Failed") {
    NavigationStack { SCR17Conversation(state: .failed) }.environment(MockApp())
}

#Preview("SCR-17 About a result") {
    NavigationStack { SCR17Conversation(state: .aboutResult) }.environment(MockApp())
}
