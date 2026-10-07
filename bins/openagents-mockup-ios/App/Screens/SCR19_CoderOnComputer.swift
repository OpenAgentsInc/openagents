import SwiftUI

// SCR-19 Chat: Coder on a computer. Coder's reply streams in, what it ran,
// a delegated session, a question with Approve / Deny, Stop, Edit queue.

enum SCR19State: String, Hashable, CaseIterable {
    case working, asked, done
}

struct SCR19CoderOnComputer: View {
    @Environment(MockApp.self) private var app
    let state: SCR19State
    @State private var lines: [String] = []
    @State private var commandShown = false
    @State private var asked = false
    @State private var answered: String? = nil
    @State private var phase = "Queued"
    @State private var delegatedOpen = false
    @State private var text = ""
    @State private var queued: [String] = []
    @State private var started = false

    var body: some View {
        VStack(spacing: 0) {
            // E01
            ChatHeader(back: BackControl(label: "") { app.back() },
                       onMenu: { app.go(.previousChats(.normal)) },
                       title: "\(phase) · \(MockData.computerName)") {
                Button { app.go(.newChat(.returning)) } label: {
                    Image(systemName: "square.and.pencil").font(.paper(19, weight: .semibold))
                        .frame(width: 44, height: 44)
                }
            }

            ScrollView {
                VStack(alignment: .leading, spacing: Theme.Space.m) {
                    Bubble(text: MockData.coderRequest, mine: true)
                    // E02
                    ForEach(lines, id: \.self) { Bubble(text: $0, mine: false) }
                    if lines.isEmpty { ThinkingLabel() }
                    // E03
                    if commandShown {
                        HStack {
                            Text(MockData.coderCommand).font(Theme.Fonts.mono)
                            Spacer()
                            Image(systemName: "checkmark").font(.paper(15, weight: .bold))
                        }
                        .padding(12)
                        .background(RoundedRectangle(cornerRadius: 10).fill(Theme.Colors.surface))
                        .overlay(RoundedRectangle(cornerRadius: 10).stroke(Theme.Colors.stroke, lineWidth: 1))
                        // E04
                        Button { withAnimation { delegatedOpen.toggle() } } label: {
                            HStack {
                                Image(systemName: "arrow.turn.down.right")
                                Text("Delegated to \(MockData.delegatedTo)").font(Theme.Fonts.bodyBold)
                                Spacer()
                                Image(systemName: delegatedOpen ? "chevron.down" : "chevron.right")
                            }
                            .foregroundStyle(Theme.Colors.textPrimary)
                            .padding(12)
                            .background(RoundedRectangle(cornerRadius: 10).stroke(Theme.Colors.stroke, lineWidth: 1))
                        }
                        if delegatedOpen {
                            Text("OpenCode: Read tests/login.rs. Updated the redirect fixture. 1 file changed.")
                                .font(Theme.Fonts.caption).foregroundStyle(Theme.Colors.textSecondary)
                                .padding(.leading, 24)
                        }
                    }
                    // E05, E06
                    if asked {
                        VStack(alignment: .leading, spacing: Theme.Space.s) {
                            Text("Coder asked: \(MockData.coderQuestion)").font(Theme.Fonts.bodyBold)
                            if let answered {
                                Text("You answered: \(answered)").font(Theme.Fonts.body)
                                    .foregroundStyle(Theme.Colors.textSecondary)
                            } else {
                                HStack(spacing: Theme.Space.s) {
                                    Chip(text: "Approve", filled: true) { answer("Approve") }
                                    Chip(text: "Deny") { answer("Deny") }
                                }
                            }
                        }
                        .padding(Theme.Space.s)
                        .background(RoundedRectangle(cornerRadius: 12).fill(Theme.Colors.surface))
                    }
                    ForEach(queued, id: \.self) { q in
                        HStack { Spacer(); Text("Queued: \(q)").font(Theme.Fonts.caption).foregroundStyle(Theme.Colors.textTertiary) }
                    }
                }
                .padding(.horizontal, Theme.Space.page)
                .padding(.vertical, Theme.Space.m)
            }

            // E07, E08
            VStack(spacing: 6) {
                Composer(placeholder: asked && answered == nil ? "Answer Coder" : "Message Coder", text: $text) {
                    queued.append(text); text = ""
                }
                HStack {
                    Chip(icon: "stop.fill", text: "Stop") { phase = "Done" }
                    Chip(icon: "list.bullet", text: "Edit queue") {}
                    Spacer()
                }
            }
            .padding(.horizontal, Theme.Space.page)
            .padding(.bottom, Theme.Space.xs)
        }
        .foregroundStyle(Theme.Colors.textPrimary)
        .background(Theme.Colors.background.ignoresSafeArea())
        .task { await run() }
    }

    private func answer(_ a: String) {
        withAnimation { answered = a; phase = "Done" }
    }

    private func run() async {
        guard !started else { return }
        started = true
        if state != .working {
            lines = MockData.coderReplyLines
            commandShown = true
            asked = true
            if state == .done { answered = "Approve"; phase = "Done" } else { phase = "Working" }
            return
        }
        try? await Task.sleep(for: .seconds(0.6))
        phase = "Working"
        for line in MockData.coderReplyLines {
            try? await Task.sleep(for: .seconds(1.1))
            withAnimation { lines.append(line) }
            if lines.count == 2 { withAnimation { commandShown = true } }
        }
        try? await Task.sleep(for: .seconds(0.8))
        withAnimation { asked = true }
    }
}

#Preview("SCR-19 Coder working") {
    NavigationStack { SCR19CoderOnComputer(state: .working) }.environment(MockApp())
}

#Preview("SCR-19 Coder asked") {
    NavigationStack { SCR19CoderOnComputer(state: .asked) }.environment(MockApp())
}
