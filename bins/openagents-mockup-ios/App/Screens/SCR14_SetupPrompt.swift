import SwiftUI

// SCR-14 Just-in-time setup prompt (later). One reason, one action, a safe
// way out. Each trigger is its own variant (E04, E05, E06).

enum SCR14State: String, Hashable, CaseIterable {
    case saveProgress, connectComputer, openWallet, payConfirm
}

struct SCR14SetupPrompt: View {
    @Environment(\.dismiss) private var dismiss
    let state: SCR14State
    @State private var confirming = false

    private var title: String {
        switch state {
        case .saveProgress: "Save your progress"
        case .connectComputer: "Connect your computer"
        case .openWallet: "You earned a reward"
        case .payConfirm: "Pay a trainer"
        }
    }

    private var reason: String {
        switch state {
        case .saveProgress: "You're level 3. Save your progress so you can get it back on a new phone."
        case .connectComputer: "To work on your own code, connect your computer."
        case .openWallet: "You earned a reward. Open your wallet to keep it."
        case .payConfirm: "Pay 500 to Trainer 2PX? You can't undo this."
        }
    }

    private var action: String {
        switch state {
        case .saveProgress: "Save my progress"
        case .connectComputer: "Connect my computer"
        case .openWallet: "Open my wallet"
        case .payConfirm: "Pay"
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Theme.Space.m) {
            Text(title).condensedTitle(Theme.Fonts.screenTitle, tracking: 1.2)
                .padding(.top, Theme.Space.s)
            Divider().overlay(Theme.Colors.divider)
            // E01
            Text(reason).font(Theme.Fonts.title).fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 0)
            // E02
            PrimaryButton(title: action) {
                if state == .saveProgress { confirming = true } else { dismiss() }
            }
            // E03
            SecondaryLink(title: state == .payConfirm ? "Cancel" : "Not now") { dismiss() }
        }
        .foregroundStyle(Theme.Colors.textPrimary)
        .padding(Theme.Space.page)
        .background(Theme.Colors.surface.ignoresSafeArea())
        .alert("Keep these words secret", isPresented: $confirming) {
            Button("Show my words") { dismiss() }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("Anyone with them can use your progress. Write them down somewhere safe.")
        }
    }
}

#Preview("SCR-14 Save your progress") {
    SCR14SetupPrompt(state: .saveProgress)
}

#Preview("SCR-14 Connect your computer") {
    SCR14SetupPrompt(state: .connectComputer)
}

#Preview("SCR-14 Open your wallet") {
    SCR14SetupPrompt(state: .openWallet)
}
