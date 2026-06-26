import SwiftUI

/// A quiet, dark, "alive" backdrop — the new Onyx-style aesthetic for Khala.
///
/// Built with native SwiftUI only (`TimelineView` + `Canvas`); no third-party
/// graphics deps. Concentric rings drift slowly when idle and breathe with the
/// live mic level while recording. The accent color tracks the voice state.
struct AnimatedBackground: View {
    /// Live, smoothed mic level 0...1 (0 when idle).
    var level: Double
    /// Current state accent color.
    var accent: Color

    var body: some View {
        TimelineView(.animation) { timeline in
            let t = timeline.date.timeIntervalSinceReferenceDate
            Canvas { context, size in
                let center = CGPoint(x: size.width / 2, y: size.height * 0.42)
                let maxRadius = max(size.width, size.height) * 0.9

                // Deep background wash.
                context.fill(
                    Path(CGRect(origin: .zero, size: size)),
                    with: .color(Color(white: 0.04))
                )

                // Concentric rings: slow ambient drift + mic-driven breathing.
                let ringCount = 6
                for i in 0..<ringCount {
                    let phase = t * 0.25 + Double(i) * 0.55
                    let breathe = (sin(phase) + 1) / 2 // 0...1
                    let base = Double(i + 1) / Double(ringCount)
                    let radius = maxRadius * base * (0.55 + 0.10 * breathe + 0.35 * level)
                    let opacity = (0.10 - Double(i) * 0.012) + level * 0.10
                    let rect = CGRect(
                        x: center.x - radius,
                        y: center.y - radius,
                        width: radius * 2,
                        height: radius * 2
                    )
                    context.stroke(
                        Path(ellipseIn: rect),
                        with: .color(accent.opacity(max(0, opacity))),
                        lineWidth: 1.5
                    )
                }

                // Soft central glow that intensifies with the voice.
                let glowRadius = maxRadius * (0.18 + 0.22 * level)
                let glowRect = CGRect(
                    x: center.x - glowRadius,
                    y: center.y - glowRadius,
                    width: glowRadius * 2,
                    height: glowRadius * 2
                )
                context.fill(
                    Path(ellipseIn: glowRect),
                    with: .radialGradient(
                        Gradient(colors: [accent.opacity(0.18 + level * 0.22), .clear]),
                        center: center,
                        startRadius: 0,
                        endRadius: glowRadius
                    )
                )
            }
            .ignoresSafeArea()
        }
    }
}

#Preview {
    AnimatedBackground(level: 0.4, accent: .red)
}
