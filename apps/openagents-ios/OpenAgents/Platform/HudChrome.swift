import UIKit

// The new landing's HUD (apps/openagents-web/newland.coax) in UIKit: its
// chamfered frames, buttons and section headings, from the same geometry and
// greys (verse/hud/frame.coax, compiled into Hud.c). Strokes are about a
// point wide with a soft glow, as the lander's lines are; text that is
// chrome (buttons, headings, the wordmark) is Michroma in capitals, spaced.
enum Hud {
    static func grey(_ k: Int64, _ alpha: CGFloat = 1) -> UIColor { UIColor(white: CGFloat(verse_hud_tone(k)), alpha: alpha) }
    static let white = grey(0), light = grey(1), mid = grey(2), dim = grey(3)
    static let blue = UIColor(red: CGFloat(verse_hud_tone(4)), green: CGFloat(verse_hud_tone(5)), blue: CGFloat(verse_hud_tone(6)), alpha: 1)
    static let cutButton = CGFloat(verse_hud_cut(0)), cutPanel = CGFloat(verse_hud_cut(1))

    /// Michroma at size points, growing with Dynamic Type as style does.
    static func chrome(_ size: CGFloat, _ style: UIFont.TextStyle = .footnote) -> UIFont {
        let base = UIFont(name: "Michroma", size: size) ?? .monospacedSystemFont(ofSize: size, weight: .medium)
        return UIFontMetrics(forTextStyle: style).scaledFont(for: base, maximumPointSize: size * 2)
    }
    static func caps(_ text: String, _ font: UIFont, _ color: UIColor, kern: CGFloat = 0.12) -> NSAttributedString {
        NSAttributedString(string: text.uppercased(), attributes: [.font: font, .foregroundColor: color, .kern: font.pointSize * kern])
    }
    /// The outline (its two cut corners) and the two bright brackets, for a box.
    static func paths(_ b: CGRect, cut: CGFloat) -> (outline: UIBezierPath, brackets: UIBezierPath) {
        let x0 = Float(b.minX), y0 = Float(b.minY), x1 = Float(b.maxX), y1 = Float(b.maxY)
        let outline = UIBezierPath()
        for i in 0...6 {
            let p = verse_hud_frame_point(x0, y0, x1, y1, Float(cut), Int64(i))
            let at = CGPoint(x: CGFloat(p.x), y: CGFloat(p.y))
            if i == 0 { outline.move(to: at) } else { outline.addLine(to: at) }
        }
        let arm = CGFloat(verse_hud_frame_arm(Float(b.width), 1)), down = arm * CGFloat(verse_hud_arm_down())
        let brackets = UIBezierPath()
        brackets.move(to: CGPoint(x: b.minX, y: b.minY + down)); brackets.addLine(to: CGPoint(x: b.minX, y: b.minY)); brackets.addLine(to: CGPoint(x: b.minX + arm, y: b.minY))
        brackets.move(to: CGPoint(x: b.maxX, y: b.maxY - down)); brackets.addLine(to: CGPoint(x: b.maxX, y: b.maxY)); brackets.addLine(to: CGPoint(x: b.maxX - arm, y: b.maxY))
        return (outline, brackets)
    }
}

/// A HUD frame behind its content: a faint chamfered outline, bright L
/// brackets on the square corners, glass inside the cut.
final class HudFrame: UIView {
    var cut: CGFloat { didSet { setNeedsLayout() } }
    var light: CGFloat = 0.8 { didSet { restyle() } }
    var tint: UIColor = Hud.white { didSet { restyle() } }
    var glass: CGFloat = 0.03 { didSet { restyle() } }
    private let fill = CAShapeLayer(), outline = CAShapeLayer(), brackets = CAShapeLayer()

    init(cut: CGFloat) {
        self.cut = cut
        super.init(frame: .zero)
        isUserInteractionEnabled = false
        for l in [fill, outline, brackets] { layer.addSublayer(l) }
        for l in [outline, brackets] {
            l.fillColor = nil; l.lineWidth = 1; l.lineCap = .square
            l.shadowOffset = .zero; l.shadowRadius = 3
        }
        restyle()
    }
    required init?(coder: NSCoder) { fatalError() }

    private func restyle() {
        fill.fillColor = UIColor(white: 1, alpha: glass).cgColor
        outline.strokeColor = tint.withAlphaComponent(CGFloat(verse_hud_outline()) * light).cgColor
        brackets.strokeColor = tint.withAlphaComponent(min(1, light)).cgColor
        brackets.shadowColor = tint.cgColor; brackets.shadowOpacity = Float(0.55 * light)
        outline.shadowColor = tint.cgColor; outline.shadowOpacity = Float(0.2 * light)
    }
    override func layoutSubviews() {
        super.layoutSubviews()
        let b = bounds.insetBy(dx: 0.5, dy: 0.5)
        let p = Hud.paths(b, cut: cut)
        CATransaction.begin(); CATransaction.setDisableActions(true)
        fill.path = p.outline.cgPath; outline.path = p.outline.cgPath; brackets.path = p.brackets.cgPath
        CATransaction.commit()
    }
}

/// The lander's button (LOG IN, MAKE YOUR OWN): a HUD frame cut 7 points,
/// its word centred in spaced capitals. The main one is brighter.
final class HudButton: UIButton {
    private let hud = HudFrame(cut: Hud.cutButton)
    private let main: Bool
    private var word = ""

    init(_ title: String, _ id: String, main: Bool = false) {
        self.main = main
        super.init(frame: .zero)
        var c = UIButton.Configuration.plain()
        c.contentInsets = NSDirectionalEdgeInsets(top: 15, leading: 18, bottom: 15, trailing: 18)
        c.baseForegroundColor = Hud.white
        c.activityIndicatorColorTransformer = UIConfigurationColorTransformer { _ in Hud.white }
        configuration = c
        insertSubview(hud, at: 0)
        hud.glass = main ? 0.08 : 0.03
        accessibilityIdentifier = id
        setTitle(title, for: .normal)
        configurationUpdateHandler = { [weak self] b in self?.restyle(b.state) }
    }
    required init?(coder: NSCoder) { fatalError() }

    /// The word as written stays the VoiceOver label; the button shows it in capitals.
    override func setTitle(_ title: String?, for state: UIControl.State) {
        word = title ?? ""
        accessibilityLabel = word
        restyle(self.state)
    }
    private func restyle(_ state: UIControl.State) {
        guard var c = configuration else { return }
        let down = state.contains(.highlighted), off = state.contains(.disabled)
        let tone = off ? Hud.mid : Hud.white
        c.attributedTitle = AttributedString(Hud.caps(word, Hud.chrome(11), tone.withAlphaComponent(down ? 1 : 0.95)))
        configuration = c
        hud.light = off ? 0.35 : down ? 1.15 : main ? 1.0 : 0.72
        hud.tint = down ? Hud.blue : Hud.white
    }
    override func layoutSubviews() {
        super.layoutSubviews()
        hud.frame = bounds; sendSubviewToBack(hud)
    }
}

/// A panel's heading, as the lander's feed heads NOW ON THE GRID: a short
/// rule, the name in capitals, a faint rule across under them.
final class HudHeading: UIView {
    init(_ name: String) {
        super.init(frame: .zero)
        let label = UILabel()
        label.attributedText = Hud.caps(name, Hud.chrome(11, .subheadline), Hud.white)
        label.adjustsFontForContentSizeCategory = true
        label.accessibilityLabel = name
        label.accessibilityTraits = .header
        label.numberOfLines = 0
        let tick = UIView(), under = UIView()
        tick.backgroundColor = Hud.white; under.backgroundColor = Hud.mid.withAlphaComponent(0.25)
        for v in [label, tick, under] { v.translatesAutoresizingMaskIntoConstraints = false; addSubview(v) }
        NSLayoutConstraint.activate([
            tick.leadingAnchor.constraint(equalTo: leadingAnchor), tick.widthAnchor.constraint(equalToConstant: 6),
            tick.heightAnchor.constraint(equalToConstant: 1), tick.centerYAnchor.constraint(equalTo: label.centerYAnchor),
            label.leadingAnchor.constraint(equalTo: tick.trailingAnchor, constant: 6), label.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor),
            label.topAnchor.constraint(equalTo: topAnchor),
            under.topAnchor.constraint(equalTo: label.bottomAnchor, constant: 8), under.heightAnchor.constraint(equalToConstant: 1),
            under.leadingAnchor.constraint(equalTo: leadingAnchor), under.trailingAnchor.constraint(equalTo: trailingAnchor),
            under.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])
    }
    required init?(coder: NSCoder) { fatalError() }
}

/// A HUD panel: its rows in a column, inside a frame cut 10 points.
final class HudPanel: UIView {
    let column: UIStackView
    init(_ rows: [UIView], spacing: CGFloat = 14) {
        column = Look.stack(rows, spacing: spacing)
        super.init(frame: .zero)
        let hud = HudFrame(cut: Hud.cutPanel)
        for v in [hud, column] as [UIView] { v.translatesAutoresizingMaskIntoConstraints = false; addSubview(v) }
        NSLayoutConstraint.activate([
            hud.topAnchor.constraint(equalTo: topAnchor), hud.bottomAnchor.constraint(equalTo: bottomAnchor),
            hud.leadingAnchor.constraint(equalTo: leadingAnchor), hud.trailingAnchor.constraint(equalTo: trailingAnchor),
            column.topAnchor.constraint(equalTo: topAnchor, constant: 16), column.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -18),
            column.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 16), column.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -16),
        ])
    }
    required init?(coder: NSCoder) { fatalError() }
}

/// OPENAGENTS, spaced as the lander's wordmark, with what this screen is under it.
final class HudMasthead: UIView {
    init(title: String, close: UIButton?) {
        super.init(frame: .zero)
        let mark = UILabel()
        mark.attributedText = Hud.caps("OpenAgents", Hud.chrome(13), Hud.white, kern: 0.3)
        mark.isAccessibilityElement = false
        let name = UILabel()
        name.attributedText = Hud.caps(title, Hud.chrome(20, .title2), Hud.white, kern: 0.08)
        name.adjustsFontForContentSizeCategory = true; name.numberOfLines = 0
        name.accessibilityLabel = title; name.accessibilityTraits = .header
        var views: [UIView] = [mark, name]
        if let close { views.append(close) }
        for v in views { v.translatesAutoresizingMaskIntoConstraints = false; addSubview(v) }
        var c = [
            mark.topAnchor.constraint(equalTo: topAnchor, constant: 4), mark.leadingAnchor.constraint(equalTo: leadingAnchor),
            name.topAnchor.constraint(equalTo: mark.bottomAnchor, constant: 22), name.leadingAnchor.constraint(equalTo: leadingAnchor),
            name.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor), name.bottomAnchor.constraint(equalTo: bottomAnchor),
        ]
        if let close {
            c += [close.topAnchor.constraint(equalTo: topAnchor, constant: -8), close.trailingAnchor.constraint(equalTo: trailingAnchor)]
        }
        NSLayoutConstraint.activate(c)
    }
    required init?(coder: NSCoder) { fatalError() }
}
