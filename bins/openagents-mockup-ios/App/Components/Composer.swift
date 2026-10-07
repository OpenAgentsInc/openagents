import SwiftUI

/// The chat composer: "Message OpenAgents" and a round send button.
struct Composer: View {
    var placeholder = "Message OpenAgents"
    @Binding var text: String
    var focused: FocusState<Bool>.Binding? = nil
    var enabled = true
    let send: () -> Void

    var body: some View {
        HStack(alignment: .bottom, spacing: 8) {
            field
                .font(Theme.Fonts.body)
                .foregroundStyle(Theme.Colors.textPrimary)
                .tint(Theme.Colors.textPrimary)
                .lineLimit(1...6)
                .padding(.vertical, 12)
                .padding(.leading, 16)
            Button(action: send) {
                Image(systemName: "arrow.up")
                    .font(.paper(17, weight: .heavy))
                    .foregroundStyle(canSend ? Theme.Colors.primaryLabel : Theme.Colors.primaryDisabledLabel)
                    .frame(width: 36, height: 36)
                    .background(Circle().fill(canSend ? Theme.Colors.primaryFill : Theme.Colors.primaryDisabledFill))
            }
            .disabled(!canSend)
            .padding(.trailing, 6)
            .padding(.bottom, 6)
        }
        .background(RoundedRectangle(cornerRadius: Theme.Radius.composer).fill(Theme.Colors.surfaceRaised))
        .overlay(RoundedRectangle(cornerRadius: Theme.Radius.composer).stroke(Theme.Colors.strokeStrong, lineWidth: 1))
    }

    @ViewBuilder private var field: some View {
        let tf = TextField("", text: $text,
                           prompt: Text(placeholder).foregroundStyle(Theme.Colors.textSecondary),
                           axis: .vertical)
            .disabled(!enabled)
        if let focused { tf.focused(focused) } else { tf }
    }

    private var canSend: Bool {
        enabled && !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }
}

/// A chat bubble. Yours sits right in a gray bubble; ours sits left, plain.
struct Bubble: View {
    let text: String
    let mine: Bool

    var body: some View {
        HStack {
            if mine { Spacer(minLength: 50) }
            Text(text)
                .font(Theme.Fonts.body)
                .foregroundStyle(Theme.Colors.textPrimary)
                .padding(.horizontal, mine ? 14 : 0)
                .padding(.vertical, mine ? 10 : 2)
                .background {
                    if mine { RoundedRectangle(cornerRadius: Theme.Radius.bubble).fill(Theme.Colors.userBubble) }
                }
                .fixedSize(horizontal: false, vertical: true)
            if !mine { Spacer(minLength: 20) }
        }
    }
}

#Preview("Composer") {
    struct P: View {
        @State var t = ""
        var body: some View {
            VStack(spacing: 16) {
                Bubble(text: "What can you do here?", mine: true)
                Bubble(text: "We answer questions, explain things, and help you plan and write.", mine: false)
                Spacer()
                Composer(text: $t) {}
            }
            .padding()
            .background(Theme.Colors.background)
        }
    }
    return P()
}
