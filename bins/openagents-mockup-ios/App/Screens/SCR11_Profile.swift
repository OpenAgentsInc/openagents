import SwiftUI

// SCR-11 Profile. Level, XP, titles, your results, what you made, and help.

enum SCR11State: String, Hashable, CaseIterable {
    case normal, noResults, offline
}

struct SCR11Profile: View {
    @Environment(MockApp.self) private var app
    let state: SCR11State
    @State private var showLevel = true
    @State private var confirmShow = false
    @State private var advancedOpen = false

    var body: some View {
        ScreenScaffold {
            TopBar(back: BackControl(label: "Menu") { app.backToMenu() }, title: "Profile")
        } content: {
            // E01
            HStack(spacing: Theme.Space.s) {
                Avatar(size: 64)
                VStack(alignment: .leading, spacing: 4) {
                    Text(MockData.player.name).font(Theme.Fonts.title)
                    // E02
                    Text("Level \(app.level) · \(app.xp) XP · \(app.xpForNextLevel - app.xp) XP to level \(app.level + 1)")
                        .font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            XPBar(progress: Double(app.xp) / Double(app.xpForNextLevel))
            if state == .offline {
                Text("Last updated 10:42").font(Theme.Fonts.caption).foregroundStyle(Theme.Colors.textTertiary)
            }
            // E03
            HStack(spacing: 8) {
                Text("Titles:").font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                Text(MockData.player.title).condensedTitle(Theme.Fonts.sectionLabel, tracking: 1.5)
                    .padding(.horizontal, 10).padding(.vertical, 4)
                    .overlay(Capsule().stroke(Theme.Colors.strokeStrong, lineWidth: 1))
            }

            // E04: tapping a result opens its SCR-05.
            SectionLabel(text: "Your results").padding(.top, Theme.Space.xs)
            if state == .noResults {
                Text("No results yet. Your first test takes about \(MockData.runMinutes) minutes.")
                    .font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
            } else {
                VStack(spacing: 0) {
                    ForEach(MockData.yourResults) { r in
                        let o = MockData.outcome(r.outcome)
                        Button { app.go(.result(SCR05State(outcomeKey: r.outcome, added: false), nil)) } label: {
                            HStack {
                                Text(r.tool).font(Theme.Fonts.bodyBold)
                                Spacer()
                                Text("\(o.withoutCount) → \(o.withCount) of \(o.total)").font(Theme.Fonts.body)
                                Text(r.verdict).font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                                    .frame(width: 84, alignment: .leading)
                                Image(systemName: "chevron.right").font(.paper(13, weight: .bold))
                                    .foregroundStyle(Theme.Colors.textTertiary)
                            }
                            .foregroundStyle(Theme.Colors.textPrimary)
                            .padding(.vertical, 12)
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(PressStyle())
                        Divider().overlay(Theme.Colors.divider)
                    }
                }
            }

            // E10: the same as CARD-07.
            SectionLabel(text: "What you made").padding(.top, Theme.Space.xs)
            if state == .noResults {
                Text("Nothing yet. Make a tool in chat, or add a result, and it shows here.")
                    .font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
            } else {
                VStack(spacing: 0) {
                    ForEach(MockData.whatYouMade) { row in
                        HStack {
                            Text(row.text).font(Theme.Fonts.body)
                            Spacer()
                            Text("+\(row.xp) XP").font(Theme.Fonts.bodyBold)
                        }
                        .padding(.vertical, 12)
                        Divider().overlay(Theme.Colors.divider)
                    }
                }
            }

            // E05
            Toggle(isOn: Binding(get: { showLevel }, set: { newValue in
                if newValue { confirmShow = true } else { showLevel = false }
            })) {
                Text("Show my level to others").font(Theme.Fonts.body)
            }
            .tint(Theme.Colors.textPrimary)
            .padding(.top, Theme.Space.xs)
            .confirmationDialog("Show your level to everyone?", isPresented: $confirmShow, titleVisibility: .visible) {
                Button("Show my level") { showLevel = true }
                Button("Cancel", role: .cancel) {}
            }

            // E06
            OutlinedButton(title: "Report a problem", icon: "exclamationmark.bubble") { app.present(.report(.form)) }
            // E07 (later)
            if MockData.showLaterFeatures {
                DisclosureGroup(isExpanded: $advancedOpen) {
                    VStack(spacing: 0) {
                        ListRow(icon: "key", title: "Identity keys") { app.go(.stub("Identity keys")) }
                        ListRow(icon: "desktopcomputer", title: "Your computers") { app.go(.stub("Your computers")) }
                        ListRow(icon: "wallet.pass", title: "Wallet") { app.go(.stub("Wallet")) }
                        ListRow(icon: "film", title: "Replay the intro") { app.go(.cinematic(0)) }
                    }
                } label: {
                    Text("Advanced").font(Theme.Fonts.bodyBold)
                }
                .tint(Theme.Colors.textSecondary)
            }
        } bottom: {
            // E08, E09
            NextLine(text: "check a result to reach level \(app.level + 1).")
            // E09
            PrimaryButton(title: "Chat with OpenAgents") { app.go(.newChat(.returning)) }
        }
    }
}

#Preview("SCR-11 Profile") {
    NavigationStack { SCR11Profile(state: .normal) }.environment(MockApp())
}

#Preview("SCR-11 No results") {
    NavigationStack { SCR11Profile(state: .noResults) }.environment(MockApp())
}
