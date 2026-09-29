import SwiftUI

/// The white power symbol: an open ring with a bar through the gap.
struct PowerSymbol: View {
    var lineWidthRatio: CGFloat = 0.13

    var body: some View {
        GeometryReader { geo in
            let s = min(geo.size.width, geo.size.height)
            let w = s * lineWidthRatio
            ZStack {
                Circle()
                    .trim(from: 0.10, to: 0.90)
                    .stroke(style: StrokeStyle(lineWidth: w, lineCap: .round))
                    .rotationEffect(.degrees(-90))
                    .padding(w / 2 + s * 0.06)
                Capsule()
                    .frame(width: w, height: s * 0.46)
                    .offset(y: -s * 0.22)
            }
            .frame(width: s, height: s)
        }
        .aspectRatio(1, contentMode: .fit)
    }
}

/// SCR-01.E01: logo and OPENAGENTS wordmark.
/// A long press opens the hidden Screen index.
struct LogoWordmark: View {
    @Environment(MockApp.self) private var app
    var size: CGFloat = Theme.Size.logo

    var body: some View {
        HStack(spacing: 10) {
            PowerSymbol()
                .frame(width: size, height: size)
                .themeShadow(Theme.Shadows.emblem)
            Text("OPENAGENTS")
                .font(Theme.Fonts.wordmark)
                .tracking(Theme.Tracking.wordmark)
        }
        .foregroundStyle(Theme.Colors.textPrimary)
        .contentShape(Rectangle())
        .onLongPressGesture(minimumDuration: 0.6) { app.showIndex = true }
        .accessibilityLabel("OpenAgents")
        .accessibilityHint("Long press for the screen index")
    }
}

#Preview("Logo") {
    VStack(spacing: 30) {
        LogoWordmark()
        PowerSymbol().frame(width: 120).foregroundStyle(.white).themeShadow(Theme.Shadows.emblem)
    }
    .padding()
    .frame(maxWidth: .infinity, maxHeight: .infinity)
    .background(Theme.Colors.background)
    .environment(MockApp())
}
