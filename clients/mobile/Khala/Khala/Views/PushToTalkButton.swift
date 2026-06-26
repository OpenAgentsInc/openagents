import SwiftUI

/// The single large press-and-hold control. Press down starts recording;
/// release stops and transcribes. Color and a pulsing ring reflect the voice
/// state (Onyx color semantics live in `VoiceState.accentColor`).
struct PushToTalkButton: View {
    let state: VoiceState
    let level: Double
    let onPressDown: () -> Void
    let onPressUp: () -> Void

    @State private var isPressing = false

    private var diameter: CGFloat { 180 }

    var body: some View {
        ZStack {
            // Pulsing outer ring driven by mic level while recording.
            Circle()
                .stroke(state.accentColor.opacity(0.5), lineWidth: 3)
                .frame(width: diameter + 36 + CGFloat(level * 60),
                       height: diameter + 36 + CGFloat(level * 60))
                .animation(.easeOut(duration: 0.12), value: level)

            // Main button.
            Circle()
                .fill(state.accentColor.opacity(isPressing ? 0.35 : 0.20))
                .overlay(Circle().stroke(state.accentColor, lineWidth: 2))
                .frame(width: diameter, height: diameter)
                .scaleEffect(isPressing ? 0.96 : 1.0)
                .animation(.spring(response: 0.25, dampingFraction: 0.7), value: isPressing)

            Image(systemName: state.isBusy ? "waveform" : "mic.fill")
                .font(.system(size: 48, weight: .medium))
                .foregroundStyle(state.accentColor)
                .symbolEffect(.variableColor, isActive: state.isBusy)
        }
        .contentShape(Circle())
        // minimumDistance 0 = press-down fires immediately; we treat the drag
        // begin/end as press/release (Onyx-style hold).
        .gesture(
            DragGesture(minimumDistance: 0)
                .onChanged { _ in
                    guard !isPressing, !state.isBusy else { return }
                    isPressing = true
                    onPressDown()
                }
                .onEnded { _ in
                    guard isPressing else { return }
                    isPressing = false
                    onPressUp()
                }
        )
        // NOTE: never .disabled(state.isBusy) here — recording IS busy, and a
        // disabled view drops the in-flight DragGesture so .onEnded (release)
        // never fires, leaving the button stuck "on". The .onChanged guard
        // (!state.isBusy) already prevents starting a NEW capture while busy.
    }
}

#Preview {
    PushToTalkButton(state: .recording, level: 0.5, onPressDown: {}, onPressUp: {})
        .preferredColorScheme(.dark)
}
