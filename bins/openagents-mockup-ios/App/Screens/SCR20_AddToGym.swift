import SwiftUI

// SCR-20 Add to the Gym (sheet). Says exactly what becomes public before
// anything does (CHK-09), opened from CARD-04.E05 and SCR-05.E07.

enum SCR20State: String, Hashable, CaseIterable {
    case normal, adding, failed
}

struct SCR20AddToGym: View {
    @Environment(MockApp.self) private var app
    var state: SCR20State = .normal
    var outcomeKey = "better"
    /// The chat message whose result this adds.
    var messageID: UUID? = nil
    @State private var adding = false
    @State private var failed = false

    private var o: MockData.Outcome { MockData.outcome(outcomeKey) }
    private var isOwnDraft: Bool { o.testSet == "changelog" }

    var body: some View {
        VStack(alignment: .leading, spacing: Theme.Space.m) {
            // Title and close
            HStack {
                Text("Add to the Gym").condensedTitle(Theme.Fonts.screenTitle, tracking: 1.2)
                Spacer()
                Button { app.sheet = nil } label: {
                    Image(systemName: "xmark").font(.system(size: 16, weight: .bold)).frame(width: 44, height: 44)
                }
                .accessibilityLabel("Close")
            }
            Divider().overlay(Theme.Colors.divider)

            if failed {
                // PAT-01
                PAT01Inline(icon: "exclamationmark.triangle", what: "We couldn't add your result.",
                            means: "It's saved on this phone.", reassure: "Nothing is lost.", code: "PUB-02") {
                    failed = false
                    add()
                }
                Spacer(minLength: 0)
            } else {
                // E01
                Text("Everyone will see:").font(Theme.Fonts.bodyBold)
                VStack(alignment: .leading, spacing: 6) {
                    bullet(isOwnDraft ? "your tool and your \(o.total) tests, and how they're checked"
                                      : "the \(o.total) tests you ran, and how they're checked")
                    bullet(o.isCheck ? "your check: \(o.withoutCount) of \(o.total) → \(o.withCount) of \(o.total)"
                                     : "the result: \(o.withoutCount) of \(o.total) → \(o.withCount) of \(o.total)")
                    bullet("your trainer name, \(MockData.player.name)")
                }
                // E02
                Text("Coder's full work on each test stays private.")
                    .font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                // E03
                Text(o.isCheck
                     ? "Our referee confirms your check, and you earn \(MockData.xpForACheck) XP. \(MockData.checkTrainer) earns XP too."
                     : "Other trainers can run these tests to check the result. You earn XP when they confirm it.")
                    .font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                    .fixedSize(horizontal: false, vertical: true)
                Spacer(minLength: 0)
                // E04, E05
                PrimaryButton(title: adding ? "Adding…" : "Add to the Gym", enabled: !adding) { add() }
                SecondaryLink(title: "Not now") { notNow() }
            }
        }
        .padding(.horizontal, Theme.Space.page)
        .padding(.top, Theme.Space.xs)
        .padding(.bottom, Theme.Space.xs)
        .foregroundStyle(Theme.Colors.textPrimary)
        .onAppear {
            adding = state == .adding
            failed = state == .failed
        }
    }

    private func bullet(_ text: String) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Text("·").font(Theme.Fonts.bodyBold)
            Text(text).font(Theme.Fonts.body).fixedSize(horizontal: false, vertical: true)
        }
    }

    private func add() {
        adding = true
        let firstRun = app.isFirstRun
        Task {
            try? await Task.sleep(for: .seconds(0.8))
            adding = false
            let levelUp = app.confirmAdd(outcome: outcomeKey, messageID: messageID)
            app.afterAdd(levelUp: levelUp, firstRun: firstRun)
        }
    }

    /// Not now: the result stays on the phone. In the first run it still
    /// ends the guided path at SCR-01 (FLOW-01 rule 6).
    private func notNow() {
        let firstRun = app.isFirstRun
        app.sheet = nil
        if firstRun {
            Task { @MainActor in
                try? await Task.sleep(for: .seconds(0.4))
                app.backToMenu()
            }
        }
    }
}

#Preview("SCR-20 Add to the Gym") {
    Color.black.sheet(isPresented: .constant(true)) {
        SCR20AddToGym().presentationDetents([.fraction(Theme.Size.addToGymSheet)])
            .presentationBackground(Theme.Colors.surface)
    }
    .environment(MockApp())
}

#Preview("SCR-20 A check") {
    Color.black.sheet(isPresented: .constant(true)) {
        SCR20AddToGym(outcomeKey: "confirmed").presentationDetents([.fraction(Theme.Size.addToGymSheet)])
            .presentationBackground(Theme.Colors.surface)
    }
    .environment(MockApp())
}

#Preview("SCR-20 Adding failed") {
    Color.black.sheet(isPresented: .constant(true)) {
        SCR20AddToGym(state: .failed).presentationDetents([.fraction(Theme.Size.addToGymSheet)])
            .presentationBackground(Theme.Colors.surface)
    }
    .environment(MockApp())
}
