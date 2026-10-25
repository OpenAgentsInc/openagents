import UIKit

/// The zone launcher (ADR 0010): the Grid, the Haunted House, and the Verse
/// as Rust Native cards. Rust lists the zones and resolves a tap; this paints
/// the cards and opens what Rust names.
final class ZonesViewController: UIViewController {
    private let zones = CoreZones()
    private let scroll = UIScrollView()
    private let content = UIStackView()
    /// Sessions remain native when opened from the bundled Grid.
    var openGrid: (() -> Void)?

    /// The launcher in its own navigation stack, as a large sheet.
    static func sheet(openGrid: @escaping () -> Void) -> UIViewController {
        let zones = ZonesViewController()
        zones.openGrid = openGrid
        let nav = DrawerShellController.nav(zones)
        nav.modalPresentationStyle = .pageSheet
        if let sheet = nav.sheetPresentationController {
            sheet.detents = [.large()]
            sheet.prefersGrabberVisible = true
        }
        return nav
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        // Rust draws the "Zones" heading; the bar carries only Close.
        navigationItem.largeTitleDisplayMode = .never
        view.backgroundColor = Palette.background
        view.accessibilityIdentifier = "zones"
        navigationItem.leftBarButtonItem = UIBarButtonItem(systemItem: .close, primaryAction: UIAction { [weak self] _ in
            self?.dismiss(animated: true)
        })
        navigationItem.leftBarButtonItem?.accessibilityIdentifier = "zones-close"
        scroll.translatesAutoresizingMaskIntoConstraints = false
        scroll.alwaysBounceVertical = true
        content.translatesAutoresizingMaskIntoConstraints = false
        content.axis = .vertical
        content.spacing = 16
        view.addSubview(scroll)
        scroll.addSubview(content)
        NSLayoutConstraint.activate([
            scroll.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            scroll.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            scroll.topAnchor.constraint(equalTo: view.topAnchor),
            scroll.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            content.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor, constant: 20),
            content.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor, constant: -20),
            content.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor, constant: 12),
            content.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor, constant: -24),
            content.widthAnchor.constraint(equalTo: scroll.frameLayoutGuide.widthAnchor, constant: -40),
        ])
        render()
        // Pick up a newer catalog; the bundled pins stay on any failure.
        Task { @MainActor [weak self] in
            guard let zones = self?.zones else { return }
            if (try? await zones.refresh()) != nil { self?.render() }
        }
    }

    private var autoOpened = false
    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        // `-route zones -zone haunted-house` opens a zone directly (screenshots, UI tests).
        let args = ProcessInfo.processInfo.arguments
        guard !autoOpened, let i = args.firstIndex(of: "-zone"), i + 1 < args.count else { return }
        autoOpened = true
        if args[i + 1] == "grid" { return showGrid(animated: false) }
        guard let runner = try? zones.runner(zoneId: args[i + 1]) else { return }
        let web = ZoneWebViewController(runner: runner)
        web.modalPresentationStyle = .fullScreen
        present(web, animated: false)
    }

    private func render() {
        guard let json = try? zones.launcher() else { return }
        RustNativeCard.mount(json, in: content) { [weak self] instance, revision, key in
            self?.activate(instance, revision, key)
        }
    }

    private func activate(_ instance: String, _ revision: UInt64, _ key: String) {
        do {
            switch try zones.activate(instance: instance, revision: revision, node: key) {
            case .grid:
                showGrid(animated: true)
            case let .web(zoneId):
                let runner = try zones.runner(zoneId: zoneId)
                let web = ZoneWebViewController(runner: runner)
                web.modalPresentationStyle = .fullScreen
                present(web, animated: true)
            }
        } catch {
            Toast.show(error.localizedDescription, in: view.window)
            render()
        }
    }

    private func showGrid(animated: Bool) {
        let grid = BundledGridViewController { [weak self] in
            let open = self?.openGrid
            self?.dismiss(animated: true) { open?() }
        }
        grid.modalPresentationStyle = .fullScreen
        present(grid, animated: animated)
    }
}

/// Paints the small Rust Native trees the zone screens use (stacks, text,
/// buttons) with the app's palette. Activations go back to Rust with the
/// tree's instance and revision.
enum RustNativeCard {
    static func mount(_ json: String, in container: UIStackView, activate: @escaping (String, UInt64, String) -> Void) {
        container.arrangedSubviews.forEach { $0.removeFromSuperview() }
        guard let data = json.data(using: .utf8),
              let doc = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let root = doc["root"] as? [String: Any], let instance = doc["instance"] as? String,
              let revision = (doc["revision"] as? NSNumber)?.uint64Value else { return }
        container.addArrangedSubview(view(root, depth: 0, instance: instance, revision: revision, activate: activate))
    }

    private static func view(_ node: [String: Any], depth: Int, instance: String, revision: UInt64, activate: @escaping (String, UInt64, String) -> Void) -> UIView {
        let element = node["element"] as? [String: Any] ?? [:]
        let props = element["props"] as? [String: Any] ?? [:]
        let style = node["style"] as? [String: Any] ?? [:]
        let key = node["key"] as? String ?? ""
        switch element["kind"] as? String {
        case "stack":
            let stack = UIStackView()
            stack.axis = props["axis"] as? String == "horizontal" ? .horizontal : .vertical
            stack.spacing = space(style["gap"]) ?? 8
            stack.alignment = .fill
            stack.accessibilityIdentifier = key
            for child in props["children"] as? [[String: Any]] ?? [] {
                stack.addArrangedSubview(view(child, depth: depth + 1, instance: instance, revision: revision, activate: activate))
            }
            let pad = space(style["padding_top"]) ?? 0
            guard pad > 0 || style["radius"] != nil else { return stack }
            // A padded, rounded stack is a card on the app's card surface.
            let card = UIView()
            card.backgroundColor = Palette.card
            card.layer.cornerRadius = CGFloat((style["radius"] as? NSNumber)?.doubleValue ?? 16)
            card.layer.cornerCurve = .continuous
            card.layer.borderWidth = 1
            card.layer.borderColor = Palette.hairline.cgColor
            stack.translatesAutoresizingMaskIntoConstraints = false
            card.addSubview(stack)
            NSLayoutConstraint.activate([
                stack.leadingAnchor.constraint(equalTo: card.leadingAnchor, constant: space(style["padding_start"]) ?? pad),
                stack.trailingAnchor.constraint(equalTo: card.trailingAnchor, constant: -(space(style["padding_end"]) ?? pad)),
                stack.topAnchor.constraint(equalTo: card.topAnchor, constant: pad),
                stack.bottomAnchor.constraint(equalTo: card.bottomAnchor, constant: -(space(style["padding_bottom"]) ?? pad)),
            ])
            return card
        case "text":
            let label = UILabel()
            label.text = props["value"] as? String
            label.numberOfLines = 0
            label.adjustsFontForContentSizeCategory = true
            switch props["role"] as? String {
            case "heading":
                label.font = Fonts.ui(.sansSemibold, key == "title" ? 28 : 20)
                label.textColor = Palette.text
                label.accessibilityTraits = .header
            case "status":
                label.font = Fonts.ui(.sans, 14)
                label.textColor = Palette.secondary
            default:
                label.font = Fonts.ui(.sans, 16)
                label.textColor = Palette.text
            }
            label.accessibilityIdentifier = key
            return label
        case "button":
            var config = UIButton.Configuration.filled()
            config.title = props["label"] as? String
            config.baseBackgroundColor = Palette.accent
            config.baseForegroundColor = Palette.background
            config.cornerStyle = .capsule
            let button = UIButton(configuration: config)
            button.isEnabled = props["enabled"] as? Bool ?? false
            button.accessibilityIdentifier = key
            button.addAction(UIAction { _ in activate(instance, revision, key) }, for: .touchUpInside)
            return button
        default:
            return UIView()
        }
    }

    /// Rust Native spacing tokens to points.
    private static func space(_ value: Any?) -> CGFloat? {
        switch value as? String {
        case "none": return 0
        case "xs": return 4
        case "sm": return 8
        case "md": return 16
        case "lg": return 24
        default: return nil
        }
    }
}
