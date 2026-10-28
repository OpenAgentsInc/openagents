import UIKit

/// Reusable native rendering/accessibility binding for the engine's slots.
/// Coax lays out every slot; callers supply symbols, action IDs and callbacks.
final class EngineHotbar: UIView {
    struct Action { let id: String; let label: String; let symbol: String; let invoke: () -> Void }
    private var controls: [UIButton] = []
    private var actions: [Action] = []
    var bottomInset: CGFloat = 0 { didSet { setNeedsLayout() } }
    func setActions(_ actions: [Action]) {
        for b in controls { b.removeFromSuperview() }; controls = []; self.actions = actions
        for action in actions {
            let b = UIButton(type: .custom)
            b.setImage(UIImage(systemName: action.symbol), for: .normal); b.tintColor = .white
            b.backgroundColor = UIColor(white: 0.02, alpha: 0.65); b.layer.cornerRadius = 10
            b.layer.borderWidth = 1; b.layer.borderColor = UIColor(white: 0.8, alpha: 0.4).cgColor
            b.accessibilityIdentifier = action.id; b.accessibilityLabel = action.label
            b.addAction(UIAction { _ in action.invoke() }, for: .touchUpInside)
            addSubview(b); controls.append(b)
        }
        setNeedsLayout()
    }
    override func layoutSubviews() {
        super.layoutSubviews()
        for (i, b) in controls.enumerated() {
            let r = metareal_hud_slot(Float(bounds.width), Float(bounds.height), Float(bottomInset), Int64(i), Int64(controls.count))
            b.frame = CGRect(x: CGFloat(r.x), y: CGFloat(r.y), width: CGFloat(r.width), height: CGFloat(r.height))
        }
    }
    override func point(inside point: CGPoint, with event: UIEvent?) -> Bool { controls.contains { !$0.isHidden && $0.frame.contains(point) } }
}
