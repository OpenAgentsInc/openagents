import SwiftUI

// CIN-01 Intro cinematic. A placeholder sequence: drawn art with slow
// pans, the narration as subtitles, Skip, and the end card. No 3D.
// Timings come from MockData.shots × MockData.cinematicTimeScale.

struct CIN01IntroCinematic: View {
    @Environment(MockApp.self) private var app
    /// Start at a given shot (the Screen index uses this to open the end card).
    var startAt = 0
    @State private var index = 0
    @State private var started = false

    private var shot: MockData.Shot { MockData.shots[index] }
    private var atEnd: Bool { shot.art == .endCard }

    var body: some View {
        ZStack {
            Color.black.ignoresSafeArea()

            ShotArtView(art: shot.art, seconds: shot.seconds * MockData.cinematicTimeScale)
                .id(shot.id)
                .transition(.opacity)
                .ignoresSafeArea()

            VStack {
                // Top: shot id (quiet) and Skip.
                HStack {
                    Text("CIN-01.\(shot.id)")
                        .font(Theme.Fonts.captionMono)
                        .foregroundStyle(Theme.Colors.textTertiary)
                    Spacer()
                    if MockData.cinematicSkipAlways && !atEnd {
                        Button("Skip") { withAnimation { index = MockData.shots.count - 1 } }
                            .font(Theme.Fonts.bodyBold)
                            .foregroundStyle(Theme.Colors.textPrimary)
                            .padding(.horizontal, 16).padding(.vertical, 8)
                            .background(Capsule().fill(Color.black.opacity(0.5)))
                            .overlay(Capsule().stroke(Theme.Colors.stroke, lineWidth: 1))
                    }
                }
                .padding(.horizontal, Theme.Space.page)
                .padding(.top, Theme.Space.xs)

                Spacer()

                if atEnd {
                    endCard.transition(.opacity.combined(with: .move(edge: .bottom)))
                }

                // Subtitle band: always on, large, white on a dark band.
                Text(shot.subtitle)
                    .font(Theme.Fonts.subtitleBand)
                    .foregroundStyle(.white)
                    .multilineTextAlignment(.center)
                    .frame(maxWidth: .infinity)
                    .padding(.horizontal, Theme.Space.l)
                    .padding(.vertical, Theme.Space.m)
                    .background(Color.black.opacity(0.72))
                    .id("sub-" + shot.id)
                    .transition(.opacity)
            }
        }
        .toolbar(.hidden, for: .navigationBar)
        .statusBarHidden()
        .task {
            guard !started else { return }
            started = true
            index = startAt
            while index < MockData.shots.count - 1 {
                try? await Task.sleep(for: .seconds(MockData.shots[index].seconds * MockData.cinematicTimeScale))
                if Task.isCancelled || index >= MockData.shots.count - 1 { break }
                withAnimation(.easeInOut(duration: 0.8)) { index += 1 }
            }
        }
    }

    private var endCard: some View {
        VStack(spacing: Theme.Space.s) {
            Text("STEP 2 OF 3").condensedTitle(Theme.Fonts.sectionLabel, tracking: Theme.Tracking.sectionLabel)
                .foregroundStyle(Theme.Colors.textSecondary)
            PrimaryButton(title: "Go to the gym") { app.go(.gym(.firstRun)) }
        }
        .padding(Theme.Space.page)
    }
}

/// One shot's placeholder art, with a slow camera move over `seconds`.
struct ShotArtView: View {
    let art: MockData.ShotArt
    let seconds: Double
    @State private var t: CGFloat = 0

    var body: some View {
        GeometryReader { geo in
            let w = geo.size.width, h = geo.size.height
            ZStack {
                switch art {
                case .emblem:
                    RadialGradient(colors: [.white.opacity(0.12 * t), .clear], center: .center, startRadius: 0, endRadius: w)
                    PowerSymbol()
                        .foregroundStyle(.white)
                        .frame(width: w * 0.3)
                        .shadow(color: .white, radius: 20 * t)
                        .shadow(color: .white.opacity(0.6), radius: 50 * t)
                        .scaleEffect(0.5 + 0.7 * t)
                        .opacity(Double(0.2 + 0.8 * t))
                case .gridCrane:
                    LinearGradient(colors: [Color(white: 0.08), .black], startPoint: .top, endPoint: .bottom)
                    GridFloor(horizon: 0.3 - 0.15 * t, lines: 30)
                        .scaleEffect(1.8 - 0.7 * t, anchor: .bottom)
                    Circle().fill(.white).frame(width: 36).shadow(color: .white, radius: 12)
                        .position(x: w * 0.55, y: h * (0.62 - 0.1 * t))
                    blocks(w: w, h: h)
                case .plaza:
                    LinearGradient(colors: [Color(white: 0.09), .black], startPoint: .top, endPoint: .bottom)
                    GridFloor(horizon: 0.36, lines: 26)
                    ForEach(0..<5, id: \.self) { i in
                        VStack(spacing: 4) {
                            Text(["Trainer 2PX", "Trainer QA4", "Trainer M0Z", "Trainer 9LT", "Trainer K2C"][i])
                                .font(Theme.Fonts.caption).foregroundStyle(.white)
                                .padding(.horizontal, 6).padding(.vertical, 2)
                                .background(Capsule().fill(.black.opacity(0.6)))
                            CoderFigure(glow: 0.5).frame(height: h * 0.16)
                        }
                        .position(x: w * (0.1 + CGFloat(i) * 0.24) - w * 0.25 * t, y: h * (0.5 + CGFloat(i % 2) * 0.08))
                    }
                case .gym:
                    LinearGradient(colors: [Color(white: 0.09), .black], startPoint: .top, endPoint: .bottom)
                    GridFloor(horizon: 0.55, lines: 24)
                    GymBuilding()
                        .frame(width: w * 0.8, height: h * 0.35)
                        .position(x: w / 2, y: h * 0.42)
                        .scaleEffect(1 + 0.8 * t, anchor: UnitPoint(x: 0.5, y: 0.55))
                case .lights:
                    FlatGrid(spacing: 22).opacity(1.5)
                    ForEach(0..<40, id: \.self) { i in
                        let a = Double(i) * 2.4
                        let r = w * 0.55 * (1 - t)
                        Circle().fill(.white).frame(width: 5)
                            .shadow(color: .white, radius: 5)
                            .position(x: w / 2 + CGFloat(cos(a)) * r * CGFloat(1 + Double(i % 5) * 0.2),
                                      y: h * 0.3 + CGFloat(sin(a)) * r * CGFloat(1 + Double(i % 3) * 0.3))
                            .opacity(Double(1 - t * 0.7))
                    }
                    PowerSymbol()
                        .foregroundStyle(.white)
                        .frame(width: w * (0.1 + 0.3 * t))
                        .shadow(color: .white, radius: 30 * t)
                        .position(x: w / 2, y: h * 0.3)
                case .loot:
                    Color.black
                    VStack(spacing: 24) {
                        Text("PLAYTESTER").condensedTitle(Theme.Fonts.headline, tracking: 4)
                            .padding(.horizontal, 16).padding(.vertical, 6)
                            .overlay(Capsule().stroke(.white, lineWidth: 2))
                            .opacity(t > 0.05 ? 1 : 0)
                        Text("LEVEL \(t > 0.5 ? 3 : 2)").font(Theme.Fonts.levelNumber)
                            .scaleEffect(0.6 + 0.4 * t)
                        Circle().fill(.white).frame(width: 60 * (0.5 + t))
                            .shadow(color: .white, radius: 30)
                    }
                    .foregroundStyle(.white)
                case .coder, .endCard:
                    LinearGradient(colors: [Color(white: 0.1), .black], startPoint: .top, endPoint: .bottom)
                    GridFloor(horizon: 0.42, lines: 24)
                    GymBuilding()
                        .frame(width: w * 0.42, height: h * 0.16)
                        .position(x: w / 2, y: h * 0.36)
                    CoderFigure()
                        .frame(height: h * 0.5)
                        .position(x: w / 2, y: h * 0.72)
                        .scaleEffect(art == .endCard ? 1 : 1.35 - 0.35 * t, anchor: .bottom)
                    if art == .endCard { Color.black.opacity(0.35) }
                }
            }
            .frame(width: w, height: h)
            .clipped()
        }
        .onAppear {
            withAnimation(.easeInOut(duration: max(seconds, 0.5))) { t = 1 }
        }
    }

    private func blocks(w: CGFloat, h: CGFloat) -> some View {
        ForEach(0..<6, id: \.self) { i in
            RoundedRectangle(cornerRadius: 2)
                .stroke(.white.opacity(0.8), lineWidth: 1)
                .frame(width: 22, height: 44)
                .position(x: w * (0.2 + CGFloat(i) * 0.05), y: h * 0.7 + CGFloat(i) * 3)
        }
    }
}

#Preview("CIN-01 Intro cinematic") {
    NavigationStack { CIN01IntroCinematic() }.environment(MockApp())
}

#Preview("CIN-01 End card") {
    NavigationStack { CIN01IntroCinematic(startAt: MockData.shots.count - 1) }.environment(MockApp())
}
