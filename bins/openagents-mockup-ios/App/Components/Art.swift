import SwiftUI
import UIKit

// Placeholder art, drawn in code: the white wireframe Grid, the Gym
// building, and Coder (a matte-black figure with a glowing power emblem).
// Drop real art into Assets.xcassets (HeroImage, CoderFigure) and the
// views below use it instead. See README.md, "Your own art".

/// The white wireframe Grid floor in perspective, fading to the horizon.
struct GridFloor: View {
    /// Where the horizon sits, 0 (top) ... 1 (bottom).
    var horizon: CGFloat = 0.38
    var lines = 22
    var color = Theme.Colors.grid

    var body: some View {
        Canvas { ctx, size in
            let hy = size.height * horizon
            let vp = CGPoint(x: size.width / 2, y: hy)
            var path = Path()
            // Lines running toward the vanishing point.
            for i in -lines...lines {
                let x = size.width / 2 + CGFloat(i) * size.width / CGFloat(lines) * 1.6
                path.move(to: CGPoint(x: x, y: size.height))
                path.addLine(to: vp)
            }
            // Cross lines, closer together near the horizon.
            for i in 1...18 {
                let t = CGFloat(i) / 18
                let y = hy + (size.height - hy) * t * t
                path.move(to: CGPoint(x: 0, y: y))
                path.addLine(to: CGPoint(x: size.width, y: y))
            }
            ctx.stroke(path, with: .color(color), lineWidth: 0.8)
        }
        .mask(
            LinearGradient(stops: [
                .init(color: .clear, location: horizon - 0.02),
                .init(color: .white.opacity(0.5), location: horizon + 0.12),
                .init(color: .white, location: 1),
            ], startPoint: .top, endPoint: .bottom)
        )
    }
}

/// A faint flat grid for backgrounds.
struct FlatGrid: View {
    var spacing: CGFloat = 32

    var body: some View {
        Canvas { ctx, size in
            var p = Path()
            var x: CGFloat = 0
            while x < size.width { p.move(to: CGPoint(x: x, y: 0)); p.addLine(to: CGPoint(x: x, y: size.height)); x += spacing }
            var y: CGFloat = 0
            while y < size.height { p.move(to: CGPoint(x: 0, y: y)); p.addLine(to: CGPoint(x: size.width, y: y)); y += spacing }
            ctx.stroke(p, with: .color(Theme.Colors.gridFaint), lineWidth: 0.6)
        }
    }
}

/// The Gym building: a white wireframe hall with a lit doorway.
struct GymBuilding: View {
    var body: some View {
        GeometryReader { geo in
            let w = geo.size.width, h = geo.size.height
            ZStack {
                // Walls
                Path { p in
                    p.addRect(CGRect(x: 0, y: h * 0.28, width: w, height: h * 0.72))
                    // Roof
                    p.move(to: CGPoint(x: -w * 0.04, y: h * 0.28))
                    p.addLine(to: CGPoint(x: w * 0.5, y: 0))
                    p.addLine(to: CGPoint(x: w * 1.04, y: h * 0.28))
                    // Columns
                    for i in 1..<6 {
                        let x = w * CGFloat(i) / 6
                        p.move(to: CGPoint(x: x, y: h * 0.34))
                        p.addLine(to: CGPoint(x: x, y: h))
                    }
                }
                .stroke(Theme.Colors.textPrimary.opacity(0.75), lineWidth: 1.2)
                // Door
                Rectangle()
                    .fill(LinearGradient(colors: [.white, .white.opacity(0.3)], startPoint: .top, endPoint: .bottom))
                    .frame(width: w * 0.2, height: h * 0.42)
                    .position(x: w / 2, y: h * 0.79)
                    .themeShadow(Theme.Shadow(color: .white.opacity(0.9), radius: 14, y: 0))
                Text("GYM")
                    .font(.paper(max(8, h * 0.11), weight: .black))
                    .tracking(2)
                    .foregroundStyle(Theme.Colors.textPrimary)
                    .position(x: w / 2, y: h * 0.22)
            }
        }
    }
}

/// Coder: a matte-black figure with a glowing power emblem on its chest.
struct CoderFigure: View {
    var glow: Double = 1

    var body: some View {
        if let image = UIImage(named: "CoderFigure") {
            Image(uiImage: image).resizable().scaledToFit()
        } else {
            GeometryReader { geo in
                let w = geo.size.width, h = geo.size.height
                ZStack {
                    // Rim light behind the figure.
                    Ellipse()
                        .fill(RadialGradient(colors: [.white.opacity(0.18 * glow), .clear], center: .center,
                                             startRadius: 0, endRadius: w * 0.7))
                        .frame(width: w * 1.4, height: h * 1.1)
                    // Legs
                    HStack(spacing: w * 0.06) {
                        Capsule().frame(width: w * 0.17, height: h * 0.36)
                        Capsule().frame(width: w * 0.17, height: h * 0.36)
                    }
                    .position(x: w / 2, y: h * 0.8)
                    // Arms
                    HStack(spacing: w * 0.52) {
                        Capsule().frame(width: w * 0.14, height: h * 0.36).rotationEffect(.degrees(8))
                        Capsule().frame(width: w * 0.14, height: h * 0.36).rotationEffect(.degrees(-8))
                    }
                    .position(x: w / 2, y: h * 0.47)
                    // Torso
                    RoundedRectangle(cornerRadius: w * 0.16)
                        .frame(width: w * 0.56, height: h * 0.4)
                        .position(x: w / 2, y: h * 0.47)
                    // Head
                    RoundedRectangle(cornerRadius: w * 0.12)
                        .frame(width: w * 0.32, height: h * 0.17)
                        .position(x: w / 2, y: h * 0.16)
                    // Visor
                    Capsule()
                        .fill(.white.opacity(0.85 * glow))
                        .frame(width: w * 0.2, height: h * 0.018)
                        .position(x: w / 2, y: h * 0.155)
                        .themeShadow(Theme.Shadow(color: .white, radius: 4, y: 0))
                    // Emblem
                    PowerSymbol()
                        .foregroundStyle(.white)
                        .frame(width: w * 0.2)
                        .position(x: w / 2, y: h * 0.43)
                        .shadow(color: .white.opacity(glow), radius: 10)
                        .shadow(color: .white.opacity(0.6 * glow), radius: 22)
                }
                .foregroundStyle(
                    LinearGradient(colors: [Color(white: 0.16), Color(white: 0.03)], startPoint: .topLeading, endPoint: .bottomTrailing)
                )
            }
            .aspectRatio(0.55, contentMode: .fit)
        }
    }
}

/// SCR-01.E05 hero: Coder looking out over the Grid and the Gym.
/// Uses the HeroImage asset if the designer adds one.
struct HeroArt: View {
    var body: some View {
        GeometryReader { geo in
            ZStack {
                if let image = UIImage(named: "HeroImage") {
                    Image(uiImage: image).resizable().scaledToFill()
                        .frame(width: geo.size.width, height: geo.size.height).clipped()
                } else {
                    LinearGradient(colors: [Color(white: 0.10), .black], startPoint: .top, endPoint: .bottom)
                    RadialGradient(colors: [.white.opacity(0.14), .clear], center: UnitPoint(x: 0.62, y: 0.36),
                                   startRadius: 0, endRadius: geo.size.width * 0.6)
                    GridFloor(horizon: 0.42)
                    GymBuilding()
                        .frame(width: geo.size.width * 0.3, height: geo.size.height * 0.28)
                        .position(x: geo.size.width * 0.66, y: geo.size.height * 0.34)
                    CoderFigure()
                        .frame(height: geo.size.height * 0.7)
                        .position(x: geo.size.width * 0.22, y: geo.size.height * 0.5)
                }
            }
        }
        .clipShape(RoundedRectangle(cornerRadius: Theme.Radius.card))
        .overlay(RoundedRectangle(cornerRadius: Theme.Radius.card).stroke(Theme.Colors.stroke, lineWidth: 1))
    }
}

/// SCR-04.E03: ten blocks, one lit per finished task.
struct ProgressBlocks: View {
    let done: Int
    var total = 10

    var body: some View {
        HStack(spacing: 6) {
            ForEach(0..<total, id: \.self) { i in
                RoundedRectangle(cornerRadius: 4)
                    .fill(i < done ? Theme.Colors.textPrimary : Theme.Colors.surfaceRaised)
                    .overlay(RoundedRectangle(cornerRadius: 4).stroke(Theme.Colors.stroke, lineWidth: 1))
                    .frame(height: Theme.Size.progressBlock)
                    .themeShadow(Theme.Shadow(color: i < done ? .white.opacity(0.5) : .clear, radius: 5, y: 0))
            }
        }
        .animation(Theme.Motion.reveal, value: done)
    }
}

#Preview("Art") {
    ScrollView {
        VStack(spacing: 20) {
            HeroArt().frame(height: Theme.Size.heroHeight)
            HStack {
                CoderFigure().frame(height: 200)
                GymBuilding().frame(width: 140, height: 110)
            }
            ProgressBlocks(done: 4)
        }
        .padding()
    }
    .background(Theme.Colors.background)
}
