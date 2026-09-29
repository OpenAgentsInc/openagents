import SwiftUI

// SCR-15 Chat: new chat, ready to type. The composer is the primary.
// The first-run chat (SCR-15.E12) is SCR17Conversation(state: .firstRun):
// it's a conversation from its first second, with CARD-01 ready.

enum SCR15State: String, Hashable, CaseIterable {
    case firstTime, returning, selectorOpen, computerChosen, computerConnecting, computerOffline
    case phoneOffline, dailyLimit, fromFirstRun
}

/// The chat header shared by SCR-15, SCR-16, SCR-17, and SCR-19.
struct ChatHeader<Trailing: View>: View {
    var back: BackControl?
    var onMenu: (() -> Void)?
    let title: String
    @ViewBuilder var trailing: Trailing

    var body: some View {
        HStack(spacing: 4) {
            if let back {
                Button(action: back.action) {
                    HStack(spacing: 2) {
                        Image(systemName: "chevron.left").font(.system(size: 17, weight: .bold))
                        if !back.label.isEmpty { Text(back.label).font(Theme.Fonts.bodyBold) }
                    }
                    .frame(minWidth: 36, minHeight: 44)
                }
            }
            if let onMenu {
                Button(action: onMenu) {
                    Image(systemName: "line.3.horizontal").font(.system(size: 18, weight: .semibold))
                        .frame(width: 40, height: 44)
                }
            }
            Text(title).font(.system(size: 19, weight: .bold)).lineLimit(1).padding(.leading, 4)
            Spacer()
            trailing
        }
        .foregroundStyle(Theme.Colors.textPrimary)
        .padding(.horizontal, Theme.Space.s)
        .frame(height: Theme.Size.topBarHeight)
        .overlay(alignment: .bottom) { Rectangle().fill(Theme.Colors.divider).frame(height: 1) }
    }
}

struct SCR15NewChat: View {
    @Environment(MockApp.self) private var app
    let state: SCR15State
    @State private var text = ""
    @State private var selectorOpen = false
    @State private var target: String? = nil   // nil = Cloud
    @FocusState private var focused: Bool

    private var computerTarget: Bool { target != nil }

    var body: some View {
        VStack(spacing: 0) {
            // E11, E01, E02, E03
            ChatHeader(back: BackControl(label: state == .fromFirstRun ? "Back" : "Menu") { app.back() },
                       onMenu: { app.go(.previousChats(.normal)) },
                       title: "OpenAgents") {
                Button { withAnimation { selectorOpen.toggle() } } label: {
                    HStack(spacing: 4) {
                        Text(target.map { "\($0) · \(MockData.workspaceName)" } ?? "Cloud")
                            .font(Theme.Fonts.caption).lineLimit(1)
                        Image(systemName: "chevron.down").font(.system(size: 11, weight: .bold))
                    }
                    .padding(.horizontal, 12).frame(height: 32)
                    .background(Capsule().fill(Theme.Colors.surfaceRaised))
                    .overlay(Capsule().stroke(Theme.Colors.stroke, lineWidth: 1))
                }
            }

            ScrollView {
                VStack(alignment: .leading, spacing: Theme.Space.s) {
                    // E05
                    VStack(alignment: .leading, spacing: 8) {
                        PowerSymbol().frame(width: 34).themeShadow(Theme.Shadows.emblem)
                            .padding(.bottom, 6)
                        Text("Ask us anything. No setup needed.").font(Theme.Fonts.title)
                        Text("We answer here, and send Coder to your computer when a job needs one.")
                            .font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                    }
                    .padding(.top, Theme.Space.xl)
                }
                .padding(.horizontal, Theme.Space.page)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .scrollDismissesKeyboard(.interactively)
            .onTapGesture { focused = false }

            VStack(alignment: .leading, spacing: Theme.Space.s) {
                statusLine
                // E06 / E04: one row that scrolls sideways, so the welcome
                // lines stay visible with the keyboard up.
                ScrollView(.horizontal) {
                    HStack(spacing: 8) {
                        if selectorOpen { targets } else { chips }
                    }
                }
                .scrollIndicators(.hidden)
                // E07, E08
                Composer(placeholder: computerTarget ? "Message OpenAgents on \(MockData.computerName)" : "Message OpenAgents",
                         text: $text, focused: $focused,
                         enabled: state != .dailyLimit && state != .computerConnecting && state != .computerOffline) {
                    send()
                }
            }
            .padding(.horizontal, Theme.Space.page)
            .padding(.bottom, Theme.Space.xs)
        }
        .foregroundStyle(Theme.Colors.textPrimary)
        .background(Theme.Colors.background.ignoresSafeArea())
        .onAppear {
            selectorOpen = state == .selectorOpen
            if [.computerChosen, .computerConnecting, .computerOffline].contains(state) { target = MockData.computerName }
            if state != .selectorOpen && state != .dailyLimit { focused = true }
        }
    }

    /// E09 and the computer/phone status lines.
    @ViewBuilder private var statusLine: some View {
        switch state {
        case .computerConnecting:
            statusRow("Connecting to \(MockData.computerName)…", live: true)
        case .computerOffline:
            statusRow("\(MockData.computerName) is offline.", live: false)
        case .phoneOffline:
            HStack {
                Text("You're offline. We'll send it when you're back.").font(Theme.Fonts.body)
                    .foregroundStyle(Theme.Colors.textSecondary)
                Spacer()
                Chip(text: "Try again") {}
            }
        case .dailyLimit:
            Text("We've answered all the messages we can for you today. Try again in 3 hours.")
                .font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
        default:
            EmptyView()
        }
    }

    private func statusRow(_ text: String, live: Bool) -> some View {
        HStack {
            StatusPill(text: text, live: live)
            Spacer()
            if computerTarget { Chip(icon: "cloud", text: "Use Cloud") { target = nil } }
        }
    }

    @ViewBuilder private var chips: some View {
        switch state {
        case .firstTime, .fromFirstRun:
            ForEach(MockData.firstTimeChips, id: \.self) { id in
                Chip(icon: "questionmark.circle", text: MockData.answer(id).question) { open(id) }
            }
        default:
            // The starter chips (Test a tool, What's new, Check a result) on Cloud.
            if !computerTarget {
                ForEach(MockData.starterChips, id: \.self) { id in
                    Chip(icon: MockData.answer(id).icon, text: MockData.answer(id).question) { open(id) }
                }
            }
            Chip(icon: "clock", text: MockData.recentChats[0]) { app.go(.coderChat(.done)) }
            Chip(icon: "clock", text: MockData.recentChats[1]) { app.go(.conversation(.answer("result"))) }
            if computerTarget {
                Chip(icon: "folder", text: "website") {}
            }
            if !computerTarget { Chip(icon: "plus", text: "Connect a computer") { app.go(.stub("Your computers")) } }
        }
    }

    @ViewBuilder private var targets: some View {
        Chip(icon: target == nil ? "checkmark" : "cloud", text: "Cloud") { target = nil; selectorOpen = false }
        Chip(icon: target != nil ? "checkmark" : "desktopcomputer", text: MockData.computerName) {
            target = MockData.computerName; selectorOpen = false
        }
        Chip(icon: "plus", text: "Connect a computer") { app.go(.stub("Your computers")) }
    }

    private func open(_ id: String) {
        app.go(.conversation(.answer(id)))
    }

    private func send() {
        let t = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !t.isEmpty else { return }
        text = ""
        if computerTarget { app.go(.coderChat(.working)) } else { app.go(.conversation(.freeText(t))) }
    }
}

#Preview("SCR-15 First time") {
    NavigationStack { SCR15NewChat(state: .firstTime) }.environment(MockApp())
}

#Preview("SCR-15 Returning") {
    NavigationStack { SCR15NewChat(state: .returning) }.environment(MockApp())
}

#Preview("SCR-15 Selector open") {
    NavigationStack { SCR15NewChat(state: .selectorOpen) }.environment(MockApp())
}

#Preview("SCR-15 Daily limit") {
    NavigationStack { SCR15NewChat(state: .dailyLimit) }.environment(MockApp())
}
