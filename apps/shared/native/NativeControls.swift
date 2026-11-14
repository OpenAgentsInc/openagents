// Firm reason: 6 (UIKit controls, text input, keyboard layout and accessibility SDK).
import UIKit

struct NativeElement {
    var id: String
    var role: String
    var label: String
    var value = ""
    var disabled = false
    var complete = ""
    var type = ""
}

/// The shared UIKit projection: keyed controls survive updates, including their
/// caret/selection. Coax owns what exists, its value and the meaning of input.
final class NativeControls: UIViewController {
    let stack = UIStackView()
    private(set) var controls: [String:UIView] = [:]
    var press: ((String)->Void)?
    var edit: ((String,String)->Void)?
    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground
        let scroll = UIScrollView(); scroll.translatesAutoresizingMaskIntoConstraints = false
        scroll.keyboardDismissMode = .interactive
        // A fixed native form consumes pinches. Otherwise the two touches can
        // reach different buttons even though the scroll view has no zoom view.
        scroll.addGestureRecognizer(UIPinchGestureRecognizer(target:nil,action:nil))
        view.addSubview(scroll)
        stack.axis = .vertical; stack.spacing = 16; stack.translatesAutoresizingMaskIntoConstraints = false
        scroll.addSubview(stack)
        NSLayoutConstraint.activate([
            scroll.topAnchor.constraint(equalTo:view.safeAreaLayoutGuide.topAnchor),
            scroll.bottomAnchor.constraint(equalTo:view.keyboardLayoutGuide.topAnchor),
            scroll.leadingAnchor.constraint(equalTo:view.leadingAnchor), scroll.trailingAnchor.constraint(equalTo:view.trailingAnchor),
            stack.topAnchor.constraint(equalTo:scroll.contentLayoutGuide.topAnchor,constant:24),
            stack.bottomAnchor.constraint(equalTo:scroll.contentLayoutGuide.bottomAnchor,constant:-24),
            stack.leadingAnchor.constraint(equalTo:scroll.frameLayoutGuide.leadingAnchor,constant:20),
            stack.trailingAnchor.constraint(equalTo:scroll.frameLayoutGuide.trailingAnchor,constant:-20),
        ])
    }
    func render(_ elements:[NativeElement]) {
        loadViewIfNeeded()
        var kept:[String:UIView] = [:]
        for e in elements {
            let old=controls[e.id]
            let matches = e.role == "button" ? old is UIButton : e.role == "field" ? old is UITextField : old is UILabel
            let control = matches ? old! : make(e)
            if let b=control as? UIButton { b.configuration?.title=e.label; b.isEnabled = !e.disabled }
            else if let f=control as? UITextField {
                f.placeholder=e.label; f.accessibilityLabel=e.label; f.isEnabled = !e.disabled
                if f.text != e.value { f.text=e.value }
                f.keyboardType = e.type == "email" ? .emailAddress : e.type == "tel" ? .numberPad : .default
                f.textContentType = e.complete == "one-time-code" ? .oneTimeCode : e.complete == "email" ? .emailAddress : nil
            } else if let l=control as? UILabel {
                l.text=e.label
                l.font = .preferredFont(forTextStyle:e.role == "heading" ? .largeTitle : e.role == "notice" ? .footnote : .body)
                l.textColor = e.role == "notice" ? .secondaryLabel : .label
            }
            kept[e.id]=control
        }
        for (id,old) in controls where kept[id] !== old { old.removeFromSuperview() }
        controls=kept
        for (i,e) in elements.enumerated() {
            guard let control=kept[e.id] else {continue}
            if i<stack.arrangedSubviews.count && stack.arrangedSubviews[i] === control {continue}
            stack.insertArrangedSubview(control,at:i)
        }
    }
    private func make(_ e:NativeElement)->UIView {
        let control:UIView
        if e.role == "button" {
            let b=UIButton(configuration:.bordered())
            b.addAction(UIAction { [weak self] _ in self?.press?(e.id) },for:.primaryActionTriggered)
            b.heightAnchor.constraint(greaterThanOrEqualToConstant:44).isActive=true
            control=b
        } else if e.role == "field" {
            let f=UITextField(); f.borderStyle = .roundedRect
            f.autocapitalizationType = .none; f.autocorrectionType = .no
            f.font = .preferredFont(forTextStyle:.body); f.adjustsFontForContentSizeCategory=true
            f.heightAnchor.constraint(greaterThanOrEqualToConstant:44).isActive=true
            f.addAction(UIAction { [weak self,weak f] _ in self?.edit?(e.id,f?.text ?? "") },for:.editingChanged)
            control=f
        } else {
            let l=UILabel(); l.numberOfLines=0; l.adjustsFontForContentSizeCategory=true
            if e.role == "heading" {l.accessibilityTraits.insert(.header)}
            control=l
        }
        control.accessibilityIdentifier=e.id
        return control
    }
}
