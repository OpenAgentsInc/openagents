import SwiftUI

/// A status pill: a dot and a short line ("GYM OPEN · 38 people training now").
struct StatusPill: View {
    let text: String
    var live = true

    var body: some View {
        HStack(spacing: 8) {
            Circle()
                .fill(live ? Theme.Colors.statusLive : Theme.Colors.statusOffline)
                .frame(width: 8, height: 8)
                .themeShadow(Theme.Shadow(color: live ? .white.opacity(0.8) : .clear, radius: 4, y: 0))
            Text(text)
                .font(Theme.Fonts.caption)
                .foregroundStyle(Theme.Colors.textPrimary)
                .lineLimit(1)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 7)
        .background(Capsule().fill(Color.black.opacity(0.72)))
        .overlay(Capsule().stroke(Theme.Colors.stroke, lineWidth: 1))
    }
}

/// The ₿ balance pill from the owner's reference (off by default; see MockData).
struct BalancePill: View {
    let text: String

    var body: some View {
        Text(text)
            .font(Theme.Fonts.captionMono)
            .foregroundStyle(Theme.Colors.textPrimary)
            .padding(.horizontal, 10)
            .padding(.vertical, 5)
            .background(Capsule().fill(Theme.Colors.surfaceRaised))
            .overlay(Capsule().stroke(Theme.Colors.stroke, lineWidth: 1))
    }
}

/// A small count badge (the updates bell).
struct CountBadge: View {
    let count: Int

    var body: some View {
        Text("\(count)")
            .font(.system(size: 11, weight: .heavy))
            .foregroundStyle(Theme.Colors.primaryLabel)
            .frame(minWidth: 17, minHeight: 17)
            .background(Circle().fill(Theme.Colors.primaryFill))
    }
}

/// SCR-01.E02: the updates bell with a count.
struct BellButton: View {
    let count: Int
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: "bell")
                .font(.system(size: 20, weight: .semibold))
                .foregroundStyle(Theme.Colors.textPrimary)
                .frame(width: 44, height: 44)
                .overlay(alignment: .topTrailing) {
                    if count > 0 { CountBadge(count: count).offset(x: -4, y: 4) }
                }
        }
        .buttonStyle(PressStyle())
    }
}

#Preview("Pills") {
    VStack(spacing: 16) {
        StatusPill(text: "GYM OPEN · 38 people training now")
        StatusPill(text: "Studio Mac is offline", live: false)
        BalancePill(text: "₿ 0.00012")
        BellButton(count: 2) {}
    }
    .padding()
    .frame(maxHeight: .infinity)
    .background(Theme.Colors.background)
}
