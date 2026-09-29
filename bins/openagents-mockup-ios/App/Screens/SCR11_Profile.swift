import SwiftUI

// SCR-11 Profile. Level, XP, titles, your runs, and help.

enum SCR11State: String, Hashable, CaseIterable {
    case normal, noRuns, offline
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

            // E04
            SectionLabel(text: "Your runs").padding(.top, Theme.Space.xs)
            if state == .noRuns {
                Text("No runs yet. Your first one takes about 5 minutes.")
                    .font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
            } else {
                VStack(spacing: 0) {
                    ForEach(MockData.runs) { run in
                        HStack {
                            Text(run.tool).font(Theme.Fonts.bodyBold)
                            Spacer()
                            Text("\(run.before) → \(run.after)").font(Theme.Fonts.body)
                            Text(run.verdict).font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                                .frame(width: 90, alignment: .leading)
                            Text("+\(run.xp) XP").font(Theme.Fonts.caption).foregroundStyle(Theme.Colors.textSecondary)
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
            NextLine(text: "train Coder to reach level \(app.level + 1).")
            PrimaryButton(title: "Enter the gym") { app.go(.gym(.returning)) }
        }
    }
}

#Preview("SCR-11 Profile") {
    NavigationStack { SCR11Profile(state: .normal) }.environment(MockApp())
}

#Preview("SCR-11 No runs") {
    NavigationStack { SCR11Profile(state: .noRuns) }.environment(MockApp())
}
