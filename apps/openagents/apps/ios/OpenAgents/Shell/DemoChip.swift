import UIKit

/// The offline demo's way out (#12011): a "Demo · Exit" chip in a strip at
/// the top of the shell, above its pages. The strip is extra top safe area,
/// so every page and the drawer sit below it. The words come from Rust.
enum DemoChip {
    static let stripHeight: CGFloat = 40

    static func install(on root: UIViewController, app: AppModel) {
        root.loadViewIfNeeded()
        root.additionalSafeAreaInsets.top += stripHeight
        let copy = demoExitCopy()

        let strip = UIView()
        strip.translatesAutoresizingMaskIntoConstraints = false
        strip.backgroundColor = Palette.background
        strip.accessibilityIdentifier = "demo-strip"

        var config = UIButton.Configuration.filled()
        config.title = copy.chip
        config.baseBackgroundColor = Palette.chip
        config.baseForegroundColor = Palette.text
        config.cornerStyle = .capsule
        config.contentInsets = NSDirectionalEdgeInsets(top: 6, leading: 14, bottom: 6, trailing: 14)
        config.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { attributes in
            var attributes = attributes
            attributes.font = Fonts.ui(.sansMedium, 14)
            return attributes
        }
        let chip = UIButton(configuration: config)
        chip.translatesAutoresizingMaskIntoConstraints = false
        chip.accessibilityIdentifier = "demo-exit"
        chip.accessibilityLabel = copy.settingsTitle
        chip.accessibilityHint = copy.chipHint
        chip.addAction(UIAction { [weak app] _ in app?.exitDemo() }, for: .touchUpInside)
        strip.addSubview(chip)

        root.view.addSubview(strip)
        NSLayoutConstraint.activate([
            strip.leadingAnchor.constraint(equalTo: root.view.leadingAnchor),
            strip.trailingAnchor.constraint(equalTo: root.view.trailingAnchor),
            strip.bottomAnchor.constraint(equalTo: root.view.safeAreaLayoutGuide.topAnchor),
            strip.heightAnchor.constraint(equalToConstant: stripHeight),
            chip.centerXAnchor.constraint(equalTo: strip.centerXAnchor),
            chip.centerYAnchor.constraint(equalTo: strip.centerYAnchor),
        ])
    }
}
