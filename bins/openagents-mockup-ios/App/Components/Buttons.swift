import SwiftUI

/// The one white-filled primary button on a screen (spec CHK-01).
struct PrimaryButton: View {
    let title: String
    var detail: String? = nil
    var enabled = true
    var icon: String? = nil
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            VStack(spacing: 2) {
                HStack(spacing: 10) {
                    if let icon { Image(systemName: icon).font(.system(size: 18, weight: .bold)) }
                    Text(title).condensedTitle(Theme.Fonts.button)
                }
                if let detail {
                    Text(detail).font(Theme.Fonts.caption)
                }
            }
            .foregroundStyle(enabled ? Theme.Colors.primaryLabel : Theme.Colors.primaryDisabledLabel)
            .frame(maxWidth: .infinity, minHeight: Theme.Size.buttonHeight)
            .background(
                RoundedRectangle(cornerRadius: Theme.Radius.button)
                    .fill(enabled ? Theme.Colors.primaryFill : Theme.Colors.primaryDisabledFill)
            )
            .themeShadow(enabled ? Theme.Shadows.primary : Theme.Shadow(color: .clear, radius: 0, y: 0))
        }
        .buttonStyle(PressStyle())
        .disabled(!enabled)
    }
}

/// The primary look on a share link (CARD-07 SHARE WHAT YOU MADE).
struct PrimaryShareLink: View {
    let title: String
    let item: String

    var body: some View {
        ShareLink(item: item) {
            HStack(spacing: 10) {
                Image(systemName: "square.and.arrow.up").font(.system(size: 18, weight: .bold))
                Text(title).condensedTitle(Theme.Fonts.button)
            }
            .foregroundStyle(Theme.Colors.primaryLabel)
            .frame(maxWidth: .infinity, minHeight: Theme.Size.buttonHeight)
            .background(RoundedRectangle(cornerRadius: Theme.Radius.button).fill(Theme.Colors.primaryFill))
            .themeShadow(Theme.Shadows.primary)
        }
        .buttonStyle(PressStyle())
    }
}

/// An outlined, full-width secondary button.
struct OutlinedButton: View {
    let title: String
    var icon: String? = nil
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 10) {
                if let icon { Image(systemName: icon).font(.system(size: 16, weight: .semibold)) }
                Text(title).font(Theme.Fonts.bodyBold)
            }
            .foregroundStyle(Theme.Colors.textPrimary)
            .frame(maxWidth: .infinity, minHeight: Theme.Size.buttonHeight - 6)
            .overlay(
                RoundedRectangle(cornerRadius: Theme.Radius.button)
                    .stroke(Theme.Colors.stroke, lineWidth: Theme.Stroke.hairline)
            )
            .contentShape(Rectangle())
        }
        .buttonStyle(PressStyle())
    }
}

/// A gray text link for a secondary action ("Ask OpenAgents a question first").
struct SecondaryLink: View {
    let title: String
    var icon: String? = nil
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 6) {
                if let icon { Image(systemName: icon) }
                Text(title)
            }
            .font(Theme.Fonts.body)
            .foregroundStyle(Theme.Colors.textSecondary)
            .frame(maxWidth: .infinity, minHeight: 44)
            .contentShape(Rectangle())
        }
        .buttonStyle(PressStyle())
    }
}

/// Dims slightly while pressed.
struct PressStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .opacity(configuration.isPressed ? 0.7 : 1)
            .scaleEffect(configuration.isPressed ? 0.985 : 1)
            .animation(Theme.Motion.tap, value: configuration.isPressed)
    }
}

#Preview("Buttons") {
    VStack(spacing: 16) {
        PrimaryButton(title: "Start the test") {}
        PrimaryButton(title: "Start tomorrow at 9:00", enabled: false) {}
        PrimaryShareLink(title: "Share what you made", item: "Shared")
        OutlinedButton(title: "Share outside the app", icon: "square.and.arrow.up") {}
        SecondaryLink(title: "Ask OpenAgents a question first") {}
    }
    .padding()
    .frame(maxHeight: .infinity)
    .background(Theme.Colors.background)
}
