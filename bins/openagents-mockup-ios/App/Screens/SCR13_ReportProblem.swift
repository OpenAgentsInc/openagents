import SwiftUI

// SCR-13 Report a problem. A sheet, from Profile or a long press
// (in the mockup: long press the main menu's footer).

enum SCR13State: String, Hashable, CaseIterable {
    case form, formFromChat, sent, offlineSaved
}

struct SCR13ReportProblem: View {
    @Environment(\.dismiss) private var dismiss
    let state: SCR13State
    @State private var text = ""
    @State private var picture = true
    @State private var shareChat = false
    @State private var outcome: SCR13State?

    private var shown: SCR13State { outcome ?? state }

    var body: some View {
        VStack(alignment: .leading, spacing: Theme.Space.m) {
            HStack {
                Text("Report a problem").condensedTitle(Theme.Fonts.screenTitle, tracking: 1.2)
                Spacer()
                Button { dismiss() } label: {
                    Image(systemName: "xmark").font(.system(size: 17, weight: .bold))
                        .frame(width: 44, height: 44)
                }
            }

            switch shown {
            case .sent, .offlineSaved:
                Spacer()
                Image(systemName: shown == .sent ? "checkmark.circle" : "tray.and.arrow.down")
                    .font(.system(size: 48, weight: .light))
                    .frame(maxWidth: .infinity)
                Text(shown == .sent ? "Thanks. We got it. Code: \(MockData.reportCode)."
                                    : "Saved. We'll send it when you're back online.")
                    .font(Theme.Fonts.title).multilineTextAlignment(.center)
                    .frame(maxWidth: .infinity)
                Spacer()
                PrimaryButton(title: "Done") { dismiss() }
            default:
                // E01
                Text("What happened?").font(Theme.Fonts.bodyBold)
                TextField("", text: $text, prompt: Text("Tell us in your own words").foregroundStyle(Theme.Colors.textTertiary),
                          axis: .vertical)
                    .lineLimit(4...8)
                    .font(Theme.Fonts.body)
                    .padding(12)
                    .background(RoundedRectangle(cornerRadius: 12).fill(Theme.Colors.surfaceRaised))
                    .overlay(RoundedRectangle(cornerRadius: 12).stroke(Theme.Colors.stroke, lineWidth: 1))
                // E02
                Toggle("Add a picture of this screen", isOn: $picture).font(Theme.Fonts.body)
                // E05 (from chat only)
                if shown == .formFromChat {
                    Toggle("Share this chat (\(MockData.chatMessageCount) messages)", isOn: $shareChat.animation())
                        .font(Theme.Fonts.body)
                    if shareChat {
                        Card {
                            VStack(alignment: .leading, spacing: 6) {
                                Text("You'll send exactly this chat:").font(Theme.Fonts.caption)
                                    .foregroundStyle(Theme.Colors.textSecondary)
                                Text("You: What can you do here?\nOpenAgents: We answer questions, explain things…\n…and 10 more messages")
                                    .font(Theme.Fonts.caption)
                            }
                        }
                    }
                }
                Spacer(minLength: 0)
                // E03, E04
                NextLine(text: "tell us what went wrong.")
                PrimaryButton(title: "Send") { withAnimation { outcome = .sent } }
            }
        }
        .tint(Theme.Colors.textPrimary)
        .foregroundStyle(Theme.Colors.textPrimary)
        .padding(Theme.Space.page)
        .background(Theme.Colors.surface.ignoresSafeArea())
    }
}

#Preview("SCR-13 Report a problem") {
    SCR13ReportProblem(state: .form)
}

#Preview("SCR-13 From chat") {
    SCR13ReportProblem(state: .formFromChat)
}

#Preview("SCR-13 Sent") {
    SCR13ReportProblem(state: .sent)
}
