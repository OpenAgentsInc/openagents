import SwiftUI

// THEME: every color, font, size, radius, and shadow in the mockup.
//
// Change a value here and every screen follows. Nothing else in the app
// hard-codes a color or a font size: if you find one, move it here.
// The spec's style rules (docs/product/2026-09-28-app-wireframe.md,
// "Status key and visual style"): black background, white and gray
// monochrome, the primary action is the one white-filled button, text at
// least 17 pt, buttons at least 56 pt tall and full width.

enum Theme {

    // MARK: Colors

    enum Colors {
        /// The app background behind everything.
        static let background = Color(white: 0.0)
        /// Cards, rows, and sheets that sit on the background.
        static let surface = Color(white: 0.055)
        /// A card inside a card, the composer, a selected row.
        static let surfaceRaised = Color(white: 0.10)
        /// Hairline borders on outlined rows and cards.
        static let stroke = Color(white: 1.0, opacity: 0.16)
        /// Borders that must read clearly (selected tool, focused composer).
        static let strokeStrong = Color(white: 1.0, opacity: 0.55)
        /// Dividers between sections.
        static let divider = Color(white: 1.0, opacity: 0.10)

        /// Main text.
        static let textPrimary = Color.white
        /// Subtitles and supporting lines.
        static let textSecondary = Color(white: 0.64)
        /// Quiet notes ("Prepared answer", codes, the footer).
        static let textTertiary = Color(white: 0.44)

        /// The one primary button: fill and label.
        static let primaryFill = Color.white
        static let primaryLabel = Color.black
        /// The primary button while it is disabled (e.g. SEE THE RESULT before done).
        static let primaryDisabledFill = Color(white: 0.20)
        static let primaryDisabledLabel = Color(white: 0.55)

        /// The white/gray wireframe grid drawn behind hero art.
        static let grid = Color(white: 1.0, opacity: 0.22)
        static let gridFaint = Color(white: 1.0, opacity: 0.07)
        /// The glow around the power emblem and highlights.
        static let glow = Color.white

        /// Status dots. Monochrome by default; set to a green to taste.
        static let statusLive = Color.white
        static let statusOffline = Color(white: 0.40)

        /// Verdict headline tint on SCR-05 (monochrome by default).
        static let verdictBetter = Color.white
        static let verdictNoChange = Color(white: 0.80)
        static let verdictWorse = Color(white: 0.64)

        /// Your chat bubble.
        static let userBubble = Color(white: 0.16)
        /// Scrim behind overlays (Level up, the screen index).
        static let scrim = Color.black.opacity(0.78)
    }

    // MARK: Fonts
    //
    // "Condensed" titles use the system font's condensed width, which is
    // the closest built-in match to the reference's condensed uppercase
    // look. To use a custom font, add the .ttf/.otf to App/, list it under
    // UIAppFonts in project.yml, and change these to Font.custom(...).

    enum Fonts {
        /// Row titles: ENTER THE GYM, CHAT WITH OPENAGENTS.
        static let rowTitle = Font.system(size: 22, weight: .heavy).width(.condensed)
        /// Screen titles in the top bar: THE GYM, TRAINING.
        static let screenTitle = Font.system(size: 19, weight: .heavy).width(.condensed)
        /// Big headlines: CODER GOT BETTER, LEVEL UP.
        static let headline = Font.system(size: 34, weight: .black).width(.condensed)
        /// The huge numbers: 6 of 10 --> 8 of 10, the level number.
        static let hugeNumber = Font.system(size: 44, weight: .black).width(.condensed)
        static let levelNumber = Font.system(size: 120, weight: .black).width(.condensed)
        /// Primary button label.
        static let button = Font.system(size: 20, weight: .heavy).width(.condensed)
        /// Section labels: RECOMMENDED, YOUR RUNS.
        static let sectionLabel = Font.system(size: 14, weight: .bold).width(.condensed)
        /// Screen intro lines ("Give Coder a new tool.").
        static let title = Font.system(size: 26, weight: .bold)
        /// Body text and subtitles (spec: at least 17 pt).
        static let body = Font.system(size: 17)
        static let bodyBold = Font.system(size: 17, weight: .semibold)
        static let subtitle = Font.system(size: 17)
        /// Menu row subtitles. 16 keeps "Ask us anything. No setup needed."
        /// on one line; the spec asks for at least 17 pt, so this is a
        /// deliberate designer call to revisit.
        static let rowSubtitle = Font.system(size: 16)
        /// Small gray notes (codes, footer, "Prepared answer").
        static let caption = Font.system(size: 13, weight: .medium)
        static let captionMono = Font.system(size: 13, weight: .medium, design: .monospaced)
        static let mono = Font.system(size: 15, design: .monospaced)
        /// The OPENAGENTS wordmark.
        static let wordmark = Font.system(size: 20, weight: .black).width(.expanded)
        /// Cinematic subtitles (spec: large, white on a dark band).
        static let subtitleBand = Font.system(size: 22, weight: .semibold)
    }

    /// Letter spacing for uppercase condensed titles.
    enum Tracking {
        static let rowTitle: CGFloat = 0.6
        static let wordmark: CGFloat = 3.0
        static let sectionLabel: CGFloat = 1.4
    }

    // MARK: Spacing and sizes

    enum Space {
        static let xxs: CGFloat = 4
        static let xs: CGFloat = 8
        static let s: CGFloat = 12
        static let m: CGFloat = 16
        static let l: CGFloat = 20
        static let xl: CGFloat = 28
        static let xxl: CGFloat = 40
        /// Left and right page margin.
        static let page: CGFloat = 20
    }

    enum Size {
        /// Spec: buttons at least 56 pt tall.
        static let buttonHeight: CGFloat = 58
        static let rowMinHeight: CGFloat = 76
        static let rowIcon: CGFloat = 28
        static let rowIconBox: CGFloat = 40
        static let logo: CGFloat = 26
        static let avatar: CGFloat = 52
        static let xpBarHeight: CGFloat = 8
        static let heroHeight: CGFloat = 230
        static let chipHeight: CGFloat = 40
        static let topBarHeight: CGFloat = 52
        static let stepDot: CGFloat = 9
        static let progressBlock: CGFloat = 24
    }

    enum Radius {
        static let row: CGFloat = 14
        static let button: CGFloat = 14
        static let card: CGFloat = 16
        static let chip: CGFloat = 20
        static let pill: CGFloat = 100
        static let composer: CGFloat = 22
        static let bubble: CGFloat = 18
        static let sheet: CGFloat = 24
    }

    enum Stroke {
        static let hairline: CGFloat = 1
        static let selected: CGFloat = 2
    }

    // MARK: Shadows and glow

    struct Shadow {
        let color: Color
        let radius: CGFloat
        let y: CGFloat
    }

    enum Shadows {
        /// Under the white primary button.
        static let primary = Shadow(color: .white.opacity(0.18), radius: 18, y: 0)
        /// Under cards.
        static let card = Shadow(color: .black.opacity(0.6), radius: 12, y: 6)
        /// Around the power emblem.
        static let emblem = Shadow(color: .white.opacity(0.75), radius: 14, y: 0)
    }

    // MARK: Motion

    enum Motion {
        static let tap = Animation.easeOut(duration: 0.15)
        static let screen = Animation.easeInOut(duration: 0.3)
        static let reveal = Animation.spring(response: 0.55, dampingFraction: 0.8)
    }
}

extension View {
    func themeShadow(_ s: Theme.Shadow) -> some View {
        shadow(color: s.color, radius: s.radius, x: 0, y: s.y)
    }

    /// Uppercase condensed title style used by rows and buttons.
    func condensedTitle(_ font: Font = Theme.Fonts.rowTitle, tracking: CGFloat = Theme.Tracking.rowTitle) -> some View {
        self.font(font).tracking(tracking).textCase(.uppercase)
    }
}
