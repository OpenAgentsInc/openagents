import SwiftUI

// Revision 2 of the spec, kept so old playtest findings can be compared.
// SCR-03 The Gym and SCR-04 Training were retired in revision 3 (their
// jobs moved into chat as CARD-01 and CARD-03). They open only from the
// Screen index, under "Retired (rev 2)". Their words and numbers are the
// revision 2 ones on purpose; build.sh check skips this folder.

enum Rev2 {
    static let scoreBefore = 6
    static let scoreAfter = 8
    static let practiceTasks = 10
    static let xpPerRun = 50
    static let triedBy = ["project-map": 18, "code-finder": 9, "test-reader": 5]
}

/// SCR-03.E04/E05: a selectable tool card with a radio mark.
struct ToolCard: View {
    let tool: MockData.Tool
    let selected: Bool
    var loadingCounts = false
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(alignment: .top, spacing: Theme.Space.s) {
                Image(systemName: selected ? "largecircle.fill.circle" : "circle")
                    .font(.system(size: 22))
                    .foregroundStyle(selected ? Theme.Colors.textPrimary : Theme.Colors.textTertiary)
                Image(systemName: tool.icon)
                    .font(.system(size: 20, weight: .semibold))
                    .frame(width: 26)
                VStack(alignment: .leading, spacing: 4) {
                    Text(tool.name).condensedTitle(Theme.Fonts.rowTitle)
                    Text(tool.line).font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                        .multilineTextAlignment(.leading)
                        .fixedSize(horizontal: false, vertical: true)
                    if selected {
                        if loadingCounts {
                            LoadingBar(width: 120, height: 12).padding(.top, 2)
                        } else {
                            Text("Tried by \(Rev2.triedBy[tool.id] ?? 0) trainers")
                                .font(Theme.Fonts.caption).foregroundStyle(Theme.Colors.textTertiary)
                        }
                    }
                }
                Spacer(minLength: 0)
            }
            .foregroundStyle(Theme.Colors.textPrimary)
            .padding(Theme.Space.m)
            .background(RoundedRectangle(cornerRadius: Theme.Radius.card)
                .fill(selected ? Theme.Colors.surfaceRaised : Theme.Colors.surface))
            .overlay(
                RoundedRectangle(cornerRadius: Theme.Radius.card)
                    .stroke(selected ? Theme.Colors.strokeStrong : Theme.Colors.stroke,
                            lineWidth: selected ? Theme.Stroke.selected : Theme.Stroke.hairline)
            )
            .contentShape(Rectangle())
        }
        .buttonStyle(PressStyle())
    }
}

/// SCR-03.E11: the check-a-result card.
struct CheckCard: View {
    var body: some View {
        Card(highlighted: true) {
            HStack(alignment: .top, spacing: Theme.Space.s) {
                Image(systemName: "largecircle.fill.circle").font(.system(size: 22))
                Image(systemName: "checkmark.seal").font(.system(size: 20, weight: .semibold)).frame(width: 26)
                VStack(alignment: .leading, spacing: 4) {
                    HStack {
                        Text("Check a result").condensedTitle()
                        Spacer()
                        Text("+\(Rev2.xpPerRun) XP").font(Theme.Fonts.bodyBold)
                    }
                    Text("\(MockData.checkTrainer) says Code finder made Coder better. Run it again to confirm.")
                        .font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            .foregroundStyle(Theme.Colors.textPrimary)
        }
    }
}

