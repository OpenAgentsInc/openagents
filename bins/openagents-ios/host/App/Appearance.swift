// The theme Rust resolved (#11028). Rust owns the choice (Account >
// Appearance: System, Light, Dark) and resolves it against the phone's
// appearance, which this host reports; the host paints its own chrome from
// the palette Rust sends and keeps only a dark fallback for the first frame.
// Terminal panes stay Coder Noir in both looks.
import SwiftUI
import UIKit

/// Rust's `appearance::View`.
struct AppearanceState: Decodable, Equatable {
    struct Choice: Decodable, Equatable, Identifiable {
        let id: String
        let label: String
        let selected: Bool
    }
    /// Rust's `appearance::HostPalette`.
    struct Palette: Decodable, Equatable {
        let background: NativeColor
        let primary: NativeColor
        let secondary: NativeColor
        let tertiary: NativeColor
        let bubble: NativeColor
        let surface: NativeColor
        let raised: NativeColor
        let border: NativeColor
        let inline_code: NativeColor
        let link: NativeColor
        let success: NativeColor
        let failure: NativeColor
        let selection: NativeColor
        let selection_handle: NativeColor
    }
    /// `system`, `light`, or `dark`.
    let choice: String
    /// The resolved scheme: `light` or `dark`.
    let scheme: String
    let choices: [Choice]
    let palette: Palette
}

/// The colors this host paints its own chrome with.
struct AppColors: Equatable {
    let scheme: ColorScheme
    let background: Color
    let primary: Color
    let secondary: Color
    let raised: Color
    let border: Color

    /// The dark look, until Rust's first packet.
    static let dark = AppColors(scheme: .dark, background: .black, primary: .white,
                                secondary: Color(white: 0.6), raised: Color(white: 0.13),
                                border: Color(white: 0.22))

    init(scheme: ColorScheme, background: Color, primary: Color, secondary: Color,
         raised: Color, border: Color) {
        self.scheme = scheme
        self.background = background
        self.primary = primary
        self.secondary = secondary
        self.raised = raised
        self.border = border
    }

    init(_ state: AppearanceState) {
        self.init(scheme: state.scheme == "light" ? .light : .dark,
                  background: state.palette.background.color,
                  primary: state.palette.primary.color,
                  secondary: state.palette.secondary.color,
                  raised: state.palette.raised.color,
                  border: state.palette.border.color)
    }
}

private struct AppColorsKey: EnvironmentKey {
    static let defaultValue = AppColors.dark
}

extension EnvironmentValues {
    /// The resolved theme's chrome colors; set once at the app's root.
    var appColors: AppColors {
        get { self[AppColorsKey.self] }
        set { self[AppColorsKey.self] = newValue }
    }
}

@MainActor
enum SystemAppearance {
    /// Whether the phone itself is dark. The screen's traits are the
    /// system's; the app's own color scheme overrides only its windows.
    static var isDark: Bool {
        let scene = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first
        let style = scene?.screen.traitCollection.userInterfaceStyle
            ?? UITraitCollection.current.userInterfaceStyle
        return style != .light
    }
}

/// Reports the phone's appearance to Rust when it changes while the app is
/// in front (a scheduled switch at sunset, say). Coming to the front reports
/// it too (`OpenAgentsApp`).
struct SystemAppearanceWatcher: UIViewRepresentable {
    let changed: () -> Void

    func makeUIView(context: Context) -> WatcherView {
        let view = WatcherView()
        view.changed = changed
        view.isUserInteractionEnabled = false
        return view
    }

    func updateUIView(_ view: WatcherView, context: Context) { view.changed = changed }

    final class WatcherView: UIView {
        var changed: (() -> Void)?
        private var registration: (any UITraitChangeRegistration)?

        override func didMoveToWindow() {
            super.didMoveToWindow()
            guard registration == nil, let scene = window?.windowScene else { return }
            registration = scene.registerForTraitChanges([UITraitUserInterfaceStyle.self]) {
                [weak self] (_: UIWindowScene, _: UITraitCollection) in self?.changed?()
            }
        }
    }
}
