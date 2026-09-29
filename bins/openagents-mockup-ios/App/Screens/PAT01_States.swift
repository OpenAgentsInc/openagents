import SwiftUI

// PAT-01 Offline, error, and empty states. Every failure uses this: plain
// words, a reassurance only when true, one primary action, never a dead end.

enum PAT01State: String, Hashable, CaseIterable {
    case offlineGym, trainingFailed, addingFailed, ourError, offlineMenu, offlineFirstRun
}

struct PAT01States: View {
    @Environment(MockApp.self) private var app
    let state: PAT01State

    private var content: (icon: String, what: String, means: String, reassure: String?, primary: String, code: String) {
        switch state {
        case .offlineGym, .offlineFirstRun:
            ("wifi.slash", "You're offline.", "Training needs the internet.", "Nothing is lost.", "Try again", "NET-01")
        case .offlineMenu:
            ("wifi.slash", "You're offline.", "We can't load this right now.", "Nothing is lost.", "Try again", "NET-01")
        case .trainingFailed:
            ("exclamationmark.triangle", "Something went wrong on our side.", "This run didn't count against today's runs.", nil, "Try again", "RUN-03")
        case .addingFailed:
            ("exclamationmark.triangle", "We couldn't add your result.", "It's saved on this phone.", "Nothing is lost.", "Try again", "PUB-02")
        case .ourError:
            ("exclamationmark.triangle", "Something went wrong on our side.", "We couldn't load this screen.", nil, "Try again", "SRV-01")
        }
    }

    var body: some View {
        let c = content
        VStack(alignment: .leading, spacing: Theme.Space.s) {
            Spacer()
            Image(systemName: c.icon).font(.system(size: 44, weight: .light))
                .padding(.bottom, Theme.Space.s)
            // E01, E02, E03
            Text(c.what).font(Theme.Fonts.title)
            Text(c.means).font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
            if let r = c.reassure {
                Text(r).font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
            }
            Spacer()
            // E04
            PrimaryButton(title: c.primary) { retry() }
            // E05: "Back" to the current step inside the first run.
            SecondaryLink(title: state == .offlineFirstRun ? "Back" : "Back to menu") {
                if state == .offlineFirstRun { app.back() } else { app.backToMenu() }
            }
            // E06
            Text("Code: \(c.code) (for reports)")
                .font(Theme.Fonts.captionMono).foregroundStyle(Theme.Colors.textTertiary)
                .frame(maxWidth: .infinity)
        }
        .foregroundStyle(Theme.Colors.textPrimary)
        .padding(.horizontal, Theme.Space.page)
        .padding(.bottom, Theme.Space.xs)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Theme.Colors.background.ignoresSafeArea())
        .toolbar(.hidden, for: .navigationBar)
    }

    /// Try again: the mockup "succeeds" and moves to the screen that failed.
    private func retry() {
        app.back()
        switch state {
        case .offlineGym, .offlineFirstRun: app.go(.gym(.returning))
        case .trainingFailed: app.go(.training(.running))
        case .addingFailed: app.go(.result(.added))
        case .ourError, .offlineMenu: break
        }
    }
}

#Preview("PAT-01 Offline") {
    NavigationStack { PAT01States(state: .offlineGym) }.environment(MockApp())
}

#Preview("PAT-01 Our error") {
    NavigationStack { PAT01States(state: .trainingFailed) }.environment(MockApp())
}
