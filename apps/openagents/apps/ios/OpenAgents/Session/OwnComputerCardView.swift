import UIKit

/// "Run this on your own computer" (#11982): the card a cloud chat shows
/// under its first good answer. Rust decides when it shows and what it says
/// (`openagents_account::own_computer`); this view paints the copy, copies
/// the install command, and opens the links.
@MainActor
final class OwnComputerCardView: UIView {
    /// The person closed the card.
    var onClose: (() -> Void)?
    private let card: OwnComputerCard

    init(card: OwnComputerCard) {
        self.card = card
        super.init(frame: .zero)
        accessibilityIdentifier = "own-computer-card"
        backgroundColor = Palette.card
        layer.cornerRadius = 16
        layer.cornerCurve = .continuous
        layer.borderWidth = 1
        layer.borderColor = Palette.hairline.cgColor

        let title = label(card.title, Fonts.ui(.sansSemibold, 16), Palette.text)
        let close = UIButton(type: .system)
        close.setImage(UIImage(systemName: "xmark", withConfiguration: UIImage.SymbolConfiguration(pointSize: 13, weight: .semibold)), for: .normal)
        close.tintColor = Palette.secondary
        close.accessibilityLabel = "Close"
        close.accessibilityIdentifier = "own-computer-close"
        close.addAction(UIAction { [weak self] _ in self?.onClose?() }, for: .touchUpInside)
        close.setContentHuggingPriority(.required, for: .horizontal)
        let header = UIStackView(arrangedSubviews: [title, close])
        header.alignment = .top

        let command = UIButton(type: .system)
        var config = UIButton.Configuration.plain()
        config.attributedTitle = AttributedString(card.command, attributes: AttributeContainer([.font: Fonts.ui(.mono, 12), .foregroundColor: Palette.text]))
        config.image = UIImage(systemName: "doc.on.doc", withConfiguration: UIImage.SymbolConfiguration(pointSize: 12))
        config.imagePlacement = .trailing
        config.imagePadding = 8
        config.baseForegroundColor = Palette.secondary
        config.contentInsets = NSDirectionalEdgeInsets(top: 10, leading: 12, bottom: 10, trailing: 12)
        config.titleLineBreakMode = .byWordWrapping
        config.background.backgroundColor = Palette.codeBackground
        config.background.strokeColor = Palette.codeBorder
        config.background.strokeWidth = 1
        config.background.cornerRadius = 10
        command.configuration = config
        command.contentHorizontalAlignment = .leading
        command.accessibilityLabel = "Copy the install command"
        command.accessibilityValue = card.command
        command.accessibilityIdentifier = "own-computer-command"
        command.addAction(UIAction { [weak self] _ in self?.copyCommand() }, for: .touchUpInside)

        let connect = UITextView()
        connect.isEditable = false
        connect.isScrollEnabled = false
        connect.backgroundColor = .clear
        connect.textContainerInset = .zero
        connect.textContainer.lineFragmentPadding = 0
        connect.attributedText = connectText()
        connect.linkTextAttributes = [.foregroundColor: Palette.text, .underlineStyle: NSUnderlineStyle.single.rawValue]
        connect.accessibilityIdentifier = "own-computer-connect"

        let download = UIButton(type: .system)
        var link = UIButton.Configuration.plain()
        link.attributedTitle = AttributedString(card.downloadLabel, attributes: AttributeContainer([.font: Fonts.ui(.sansMedium, 14), .foregroundColor: Palette.text, .underlineStyle: NSUnderlineStyle.single]))
        link.contentInsets = .zero
        download.configuration = link
        download.contentHorizontalAlignment = .leading
        download.accessibilityIdentifier = "own-computer-download"
        download.addAction(UIAction { [weak self] _ in self?.open(self?.card.downloadUrl) }, for: .touchUpInside)

        let stack = UIStackView(arrangedSubviews: [header, label(card.body, Fonts.ui(.sans, 14), Palette.secondary), command, connect, download])
        stack.axis = .vertical
        stack.spacing = 10
        stack.setCustomSpacing(6, after: header)
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)
        NSLayoutConstraint.activate([
            stack.topAnchor.constraint(equalTo: topAnchor, constant: 14),
            stack.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 16),
            stack.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -12),
            stack.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -14),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError() }

    override func traitCollectionDidChange(_ previous: UITraitCollection?) {
        super.traitCollectionDidChange(previous)
        layer.borderColor = Palette.hairline.cgColor
    }

    private func label(_ text: String, _ font: UIFont, _ color: UIColor) -> UILabel {
        let label = UILabel()
        label.text = text
        label.font = font
        label.textColor = color
        label.numberOfLines = 0
        return label
    }

    /// The connect line, with the approval page as a link.
    private func connectText() -> NSAttributedString {
        let text = NSMutableAttributedString(string: card.connect, attributes: [.font: Fonts.ui(.sans, 14), .foregroundColor: Palette.secondary])
        let shown = card.approveUrl.replacingOccurrences(of: "https://", with: "")
        let range = (card.connect as NSString).range(of: shown)
        if range.location != NSNotFound, let url = URL(string: card.approveUrl) {
            text.addAttribute(.link, value: url, range: range)
        }
        return text
    }

    private func copyCommand() {
        UIPasteboard.general.string = card.command
        UIImpactFeedbackGenerator(style: .light).impactOccurred()
        Toast.show("Copied the install command", in: window)
    }

    private func open(_ link: String?) {
        guard let link, let url = URL(string: link) else { return }
        UIApplication.shared.open(url)
    }
}
