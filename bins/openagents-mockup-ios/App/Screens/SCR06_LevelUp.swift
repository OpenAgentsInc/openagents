import SwiftUI

// SCR-06 Level up. An overlay when XP crosses a level: after Add to the Gym
// (SCR-20), or when chat shows a new award (CARD-07).

enum SCR06State: String, Hashable, CaseIterable {
    case withTitle, noTitle
}

struct SCR06LevelUp: View {
    @Environment(MockApp.self) private var app
    let state: SCR06State
    @State private var shown = false
    @State private var fill: Double = 0

    var body: some View {
        ZStack {
            Theme.Colors.background.ignoresSafeArea()
            RadialGradient(colors: [.white.opacity(0.22), .clear], center: .center, startRadius: 0, endRadius: 320)
                .ignoresSafeArea()
                .opacity(shown ? 1 : 0)
            FlatGrid().opacity(0.6).ignoresSafeArea()

            VStack(spacing: Theme.Space.l) {
                Spacer()
                // E01
                Text("Level up").condensedTitle(Theme.Fonts.headline, tracking: 6)
                Text("\(max(app.level, MockData.player.level + 1))")
                    .font(Theme.Fonts.levelNumber)
                    .shadow(color: .white.opacity(0.8), radius: 24)
                    .scaleEffect(shown ? 1 : 0.4)
                    .opacity(shown ? 1 : 0)
                // E02
                VStack(spacing: 6) {
                    XPBar(progress: fill, height: 12)
                    Text("\(MockData.player.xpForNextLevel) XP").font(Theme.Fonts.caption)
                        .foregroundStyle(Theme.Colors.textSecondary)
                }
                .padding(.horizontal, Theme.Space.xl)
                // E03
                if state == .withTitle {
                    HStack(spacing: 8) {
                        Text("New:").font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                        Text(MockData.player.title).condensedTitle(Theme.Fonts.rowTitle, tracking: 2)
                            .padding(.horizontal, 10).padding(.vertical, 3)
                            .overlay(Capsule().stroke(.white, lineWidth: 1.5))
                        Text("title on your name").font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                    }
                    .opacity(shown ? 1 : 0)
                }
                Spacer()
                // E04, E05
                VStack(spacing: Theme.Space.xs) {
                    NextLine(text: "check someone's result for more XP.")
                    // E05: back to the chat, or SCR-01 at the end of the first run.
                    PrimaryButton(title: "Nice") {
                        if app.isFirstRun || app.path.count <= 1 { app.backToMenu() } else { app.back() }
                    }
                }
                .padding(.horizontal, Theme.Space.page)
                .padding(.bottom, Theme.Space.xs)
            }
            .foregroundStyle(Theme.Colors.textPrimary)
        }
        .toolbar(.hidden, for: .navigationBar)
        .onAppear {
            withAnimation(.easeOut(duration: 0.9)) { fill = 1 }
            withAnimation(Theme.Motion.reveal.delay(0.5)) { shown = true }
            UINotificationFeedbackGenerator().notificationOccurred(.success)
        }
    }
}

#Preview("SCR-06 Level up") {
    NavigationStack { SCR06LevelUp(state: .withTitle) }.environment(MockApp())
}

#Preview("SCR-06 No new title") {
    NavigationStack { SCR06LevelUp(state: .noTitle) }.environment(MockApp())
}
