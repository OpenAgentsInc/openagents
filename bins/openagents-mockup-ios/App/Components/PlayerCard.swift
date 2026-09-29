import SwiftUI

/// The trainer's avatar: a placeholder mark drawn in code.
struct Avatar: View {
    var size: CGFloat = Theme.Size.avatar

    var body: some View {
        ZStack {
            RoundedRectangle(cornerRadius: size * 0.24)
                .fill(LinearGradient(colors: [Color(white: 0.22), Color(white: 0.06)],
                                     startPoint: .topLeading, endPoint: .bottomTrailing))
            RoundedRectangle(cornerRadius: size * 0.24)
                .stroke(Theme.Colors.strokeStrong, lineWidth: 1)
            PowerSymbol()
                .frame(width: size * 0.46)
                .foregroundStyle(.white)
                .themeShadow(Theme.Shadow(color: .white.opacity(0.6), radius: 6, y: 0))
        }
        .frame(width: size, height: size)
    }
}

/// A level/XP bar. `progress` is 0...1; nil draws the loading gray bar.
struct XPBar: View {
    var progress: Double?
    var height: CGFloat = Theme.Size.xpBarHeight

    var body: some View {
        GeometryReader { geo in
            ZStack(alignment: .leading) {
                Capsule().fill(Theme.Colors.surfaceRaised)
                Capsule().stroke(Theme.Colors.stroke, lineWidth: 1)
                if let progress {
                    Capsule().fill(Theme.Colors.textPrimary)
                        .frame(width: max(height, geo.size.width * min(max(progress, 0), 1)))
                        .themeShadow(Theme.Shadow(color: .white.opacity(0.35), radius: 6, y: 0))
                }
            }
        }
        .frame(height: height)
        .animation(Theme.Motion.reveal, value: progress)
    }
}

/// A gray bar that stands in for a number still loading (spec: never show 0).
struct LoadingBar: View {
    var width: CGFloat = 80
    var height: CGFloat = 14
    @State private var pulse = false

    var body: some View {
        RoundedRectangle(cornerRadius: 4)
            .fill(Theme.Colors.surfaceRaised)
            .frame(width: width, height: height)
            .opacity(pulse ? 0.5 : 1)
            .onAppear {
                withAnimation(.easeInOut(duration: 0.8).repeatForever()) { pulse = true }
            }
    }
}

/// SCR-01.E04: avatar, trainer name, level, XP bar. Tapping opens Profile.
struct PlayerCard: View {
    var name = MockData.player.name
    var level: Int
    var xp: Int
    var xpForNext: Int
    var loading = false
    var action: (() -> Void)? = nil

    var body: some View {
        Button { action?() } label: {
            HStack(spacing: Theme.Space.s) {
                Avatar()
                VStack(alignment: .leading, spacing: 6) {
                    HStack {
                        Text(name).font(Theme.Fonts.bodyBold).foregroundStyle(Theme.Colors.textPrimary)
                        Spacer()
                        if MockData.showBalancePill { BalancePill(text: MockData.player.balance) }
                    }
                    HStack(spacing: 8) {
                        Text("LEVEL \(level)").condensedTitle(Theme.Fonts.sectionLabel, tracking: 1)
                            .foregroundStyle(Theme.Colors.textPrimary)
                        if loading {
                            LoadingBar(width: 140, height: 8)
                        } else {
                            XPBar(progress: Double(xp) / Double(xpForNext))
                        }
                    }
                    if loading {
                        LoadingBar(width: 70, height: 10)
                    } else {
                        Text("\(xp) / \(xpForNext) XP")
                            .font(Theme.Fonts.caption)
                            .foregroundStyle(Theme.Colors.textSecondary)
                    }
                }
            }
            .padding(Theme.Space.s)
            .background(RoundedRectangle(cornerRadius: Theme.Radius.card).fill(Theme.Colors.surface))
            .overlay(RoundedRectangle(cornerRadius: Theme.Radius.card).stroke(Theme.Colors.stroke, lineWidth: 1))
            .contentShape(Rectangle())
        }
        .buttonStyle(PressStyle())
    }
}

#Preview("Player card") {
    VStack(spacing: 20) {
        PlayerCard(level: 2, xp: 140, xpForNext: 283)
        PlayerCard(level: 2, xp: 140, xpForNext: 283, loading: true)
        XPBar(progress: 0.6)
        Avatar(size: 80)
    }
    .padding()
    .frame(maxHeight: .infinity)
    .background(Theme.Colors.background)
}
