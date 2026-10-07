import SwiftUI

// CARD-04 Result card: the payoff, in the chat. Tests passed without and
// with the tool are the biggest text; the one button adds it to the Gym
// (SCR-20), or, for a one-run try, runs the full test set.

/// The states the Screen index lists (keys into MockData.outcomes).
enum CARD04State: String, Hashable, CaseIterable {
    case better, noChange, worse, firstTry, madeBetter, confirmed, didntHold, added
    var outcomeKey: String { self == .added ? "better" : rawValue }
}

struct CARD04Result: View {
    let outcomeKey: String
    var added = false
    var onAdd: () -> Void = {}
    var onRunFull: () -> Void = {}
    var onDetails: () -> Void = {}
    var onSeeTests: () -> Void = {}

    private var o: MockData.Outcome { MockData.outcome(outcomeKey) }

    private var headlineColor: Color {
        switch outcomeKey {
        case "noChange", "didntHold": Theme.Colors.verdictNoChange
        case "worse": Theme.Colors.verdictWorse
        default: Theme.Colors.verdictBetter
        }
    }

    var body: some View {
        ChatCardFrame(highlighted: !added) {
            // E01
            Text(o.headline).condensedTitle(Theme.Fonts.cardHeadline, tracking: 0.8)
                .foregroundStyle(headlineColor)
                .fixedSize(horizontal: false, vertical: true)
            // E02
            HStack(alignment: .firstTextBaseline, spacing: 10) {
                side("without", o.withoutCount)
                Image(systemName: "arrow.right").font(.paper(20, weight: .heavy))
                side("with", o.withCount)
            }
            CardNote(text: "tests · \(o.toolName)\(o.isFirstTry ? " · one run" : "")")
            // E03
            Text(o.xpLine).font(Theme.Fonts.bodyBold).fixedSize(horizontal: false, vertical: true)

            // E05
            if added {
                Label("Added to the Gym", systemImage: "checkmark.circle.fill")
                    .font(Theme.Fonts.bodyBold)
                    .frame(maxWidth: .infinity, minHeight: 44, alignment: .leading)
            } else if o.isFirstTry {
                PrimaryButton(title: "Run the full test set") { onRunFull() }
            } else {
                PrimaryButton(title: "Add to the Gym") { onAdd() }
            }
            // E04, E06
            HStack(spacing: 8) {
                Chip(icon: "chart.bar", text: "See details") { onDetails() }
                Chip(icon: "list.bullet", text: "See the tests") { onSeeTests() }
            }
        }
    }

    private func side(_ label: String, _ n: Int) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 6) {
            Text("\(label):").font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
            Text("\(n) of \(o.total)").font(Theme.Fonts.cardNumber)
        }
        .lineLimit(1)
        .minimumScaleFactor(0.7)
    }
}

#Preview("CARD-04 Better") {
    ScrollView { CARD04Result(outcomeKey: "better").padding() }.background(Theme.Colors.background)
}

#Preview("CARD-04 No clear change") {
    ScrollView { CARD04Result(outcomeKey: "noChange").padding() }.background(Theme.Colors.background)
}

#Preview("CARD-04 Worse") {
    ScrollView { CARD04Result(outcomeKey: "worse").padding() }.background(Theme.Colors.background)
}

#Preview("CARD-04 First try") {
    ScrollView { CARD04Result(outcomeKey: "firstTry").padding() }.background(Theme.Colors.background)
}

#Preview("CARD-04 You confirmed it") {
    ScrollView { CARD04Result(outcomeKey: "confirmed").padding() }.background(Theme.Colors.background)
}

#Preview("CARD-04 It didn't hold up") {
    ScrollView { CARD04Result(outcomeKey: "didntHold").padding() }.background(Theme.Colors.background)
}

#Preview("CARD-04 Added") {
    ScrollView { CARD04Result(outcomeKey: "better", added: true).padding() }.background(Theme.Colors.background)
}
