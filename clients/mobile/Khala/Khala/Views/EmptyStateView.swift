import SwiftUI

/// ChatGPT-style empty state for a fresh Khala conversation (issue #6348).
///
/// Shown by `RootView` as a centered overlay above the chat surface when the
/// active conversation has no user/assistant messages yet. A short greeting
/// plus a few coding-oriented suggestion rows. Tapping a suggestion sends it as
/// the first user turn (the empty state then yields to the live transcript),
/// which is the boundary-safe equivalent of "prefill the composer" without
/// reaching into `ChatView`'s private composer state.
///
/// Single model: there is no variant/intelligence picker here. The pill in the
/// top bar reads "Khala" and the suggestions speak to coding work.
struct EmptyStateView: View {
    /// A canned coding prompt the user can tap to start a conversation.
    struct Suggestion: Identifiable {
        let id = UUID()
        let icon: String
        let label: String
        /// The prompt actually sent. Defaults to `label` when not specified.
        let prompt: String

        init(icon: String, label: String, prompt: String? = nil) {
            self.icon = icon
            self.label = label
            self.prompt = prompt ?? label
        }
    }

    /// Whether suggestion taps are enabled (a key is present and nothing is in
    /// flight). When disabled the rows still render but are not tappable.
    var canSend: Bool
    /// Called with the chosen prompt when a suggestion is tapped.
    var onSelect: (String) -> Void

    static let suggestions: [Suggestion] = [
        .init(icon: "curlybraces", label: "Explain this code",
              prompt: "Explain what this code does, step by step:\n\n"),
        .init(icon: "function", label: "Write a function",
              prompt: "Write a function that "),
        .init(icon: "ladybug", label: "Debug an error",
              prompt: "Help me debug this error:\n\n"),
        .init(icon: "arrow.triangle.2.circlepath", label: "Refactor a snippet",
              prompt: "Refactor this code to be cleaner and more idiomatic:\n\n")
    ]

    var body: some View {
        VStack(spacing: 28) {
            greeting
            suggestionList
        }
        .frame(maxWidth: 460)
        .padding(.horizontal, 28)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .multilineTextAlignment(.center)
        // No opaque background: this is rendered INSIDE ChatView (over the
        // AnimatedBackground), in place of the scroll content, so ChatView's
        // bottom composer inset stays visible and usable on a fresh chat.
    }

    private var greeting: some View {
        VStack(spacing: 10) {
            Image(systemName: "sparkles")
                .font(.system(size: 34, weight: .semibold))
                .foregroundStyle(.tint)
                .accessibilityHidden(true)
            Text("Khala")
                .font(.largeTitle.weight(.semibold))
                .foregroundStyle(.primary)
            Text("Collective intelligence behind a free API — one mind, many models. Ask anything, or start with one of these.")
                .font(.callout)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    private var suggestionList: some View {
        VStack(spacing: 10) {
            ForEach(Self.suggestions) { suggestion in
                Button {
                    onSelect(suggestion.prompt)
                } label: {
                    suggestionRow(suggestion)
                }
                .buttonStyle(.plain)
                .disabled(!canSend)
                .opacity(canSend ? 1 : 0.5)
                .accessibilityLabel(suggestion.label)
            }
        }
    }

    private func suggestionRow(_ suggestion: EmptyStateView.Suggestion) -> some View {
        HStack(spacing: 12) {
            Image(systemName: suggestion.icon)
                .font(.system(size: 15, weight: .semibold))
                .foregroundStyle(.tint)
                .frame(width: 26, height: 26)
                .accessibilityHidden(true)
            Text(suggestion.label)
                .font(.callout.weight(.medium))
                .foregroundStyle(.primary)
            Spacer(minLength: 0)
            Image(systemName: "arrow.up.right")
                .font(.caption.weight(.semibold))
                .foregroundStyle(.secondary)
                .accessibilityHidden(true)
        }
        .padding(.vertical, 13)
        .padding(.horizontal, 16)
        .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .stroke(.white.opacity(0.06), lineWidth: 1)
        )
    }
}

#Preview {
    ZStack {
        Color.black.ignoresSafeArea()
        EmptyStateView(canSend: true, onSelect: { _ in })
    }
    .preferredColorScheme(.dark)
}
