import SwiftUI

// PAT-01 Offline, error, and empty states. Every failure uses this: plain
// words, a reassurance only when true, one primary action, never a dead end.
// PAT01States replaces a whole screen; PAT01Inline replaces a card's body.

enum PAT01State: String, Hashable, CaseIterable {
    case offline, runFailed, addingFailed, ourError, offlineMenu, offlineFirstRun
}

struct PAT01States: View {
    @Environment(MockApp.self) private var app
    let state: PAT01State

    private var content: (icon: String, what: String, means: String, reassure: String?, primary: String, code: String) {
        switch state {
        case .offline, .offlineFirstRun:
            ("wifi.slash", "You're offline.", "Tests need the internet.", "Nothing is lost.", "Try again", "NET-01")
        case .offlineMenu:
            ("wifi.slash", "You're offline.", "We can't load this right now.", "Nothing is lost.", "Try again", "NET-01")
        case .runFailed:
            ("exclamationmark.triangle", "Something went wrong on our side.", "This didn't use a run.", nil, "Try again", "RUN-03")
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
            Image(systemName: c.icon).font(.paper(44, weight: .light))
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
        case .offline: app.go(.conversation(.answer("testATool")))
        case .offlineFirstRun: app.go(.conversation(.firstRun))
        case .runFailed: app.go(.conversation(.card(.run(.tool("project-map"), .running, Date()))))
        case .addingFailed: app.go(.result(.added, nil))
        case .ourError, .offlineMenu: break
        }
    }
}

/// PAT-01 inside a chat card (CARD-01 offline, CARD-03 failed): the same
/// four lines and one button, drawn where the card was.
struct PAT01Inline: View {
    let icon: String
    let what: String
    let means: String
    let reassure: String?
    let code: String
    var primary = "Try again"
    var onRetry: () -> Void = {}

    var body: some View {
        ChatCardFrame {
            HStack(alignment: .top, spacing: Theme.Space.s) {
                Image(systemName: icon).font(.paper(22)).frame(width: 28)
                VStack(alignment: .leading, spacing: 4) {
                    // E01, E02, E03
                    Text(what).font(Theme.Fonts.bodyBold)
                    CardNote(text: means)
                    if let reassure { CardNote(text: reassure) }
                }
            }
            // E04
            PrimaryButton(title: primary) { onRetry() }
            // E06
            Text("Code: \(code) (for reports)")
                .font(Theme.Fonts.captionMono).foregroundStyle(Theme.Colors.textTertiary)
        }
    }
}

#Preview("PAT-01 Offline") {
    NavigationStack { PAT01States(state: .offline) }.environment(MockApp())
}

#Preview("PAT-01 Run failed") {
    NavigationStack { PAT01States(state: .runFailed) }.environment(MockApp())
}

#Preview("PAT-01 Inline") {
    PAT01Inline(icon: "wifi.slash", what: "You're offline.", means: "Tests need the internet.",
                reassure: "Nothing is lost.", code: "NET-01")
        .padding().frame(maxHeight: .infinity).background(Theme.Colors.background)
}
