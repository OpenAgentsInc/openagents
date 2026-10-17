import UIKit

/// The first-launch tutorial (#11996): a few pages the person can skip,
/// ending on a new chat. There is no sign-in. Rust owns the pages' words and
/// whether the tutorial was seen (`CoreOnboarding`); this view paints them.
final class OnboardingViewController: UIViewController, UIScrollViewDelegate {
    /// The tutorial is done. `true` when the person chose "See a demo".
    var onFinish: ((Bool) -> Void)?

    private let steps: [OnboardingStep]
    private let scroll = UIScrollView()
    private let pages = UIStackView()
    private let dots = UIPageControl()
    private let primary = UIButton(configuration: .filled())
    private let skip = UIButton(type: .system)
    private let demo = UIButton(type: .system)
    private var index: Int

    init(steps: [OnboardingStep], start: Int = 0) {
        self.steps = steps
        index = max(0, min(start, steps.count - 1))
        super.init(nibName: nil, bundle: nil)
        modalPresentationStyle = .fullScreen
    }

    required init?(coder: NSCoder) { fatalError() }

    /// The SF Symbol drawn for each page, by the step's stable id.
    private static func symbol(_ id: String) -> String {
        switch id {
        case "ask": "bubble.left.and.text.bubble.right"
        case "code": "chevron.left.forwardslash.chevron.right"
        case "connect": "qrcode.viewfinder"
        case "backup": "key"
        default: "sparkles"
        }
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = Palette.background
        view.accessibilityIdentifier = "onboarding"

        scroll.isPagingEnabled = true
        scroll.showsHorizontalScrollIndicator = false
        scroll.delegate = self
        scroll.contentInsetAdjustmentBehavior = .never
        pages.axis = .horizontal
        pages.distribution = .fillEqually
        for step in steps { pages.addArrangedSubview(page(step)) }

        dots.numberOfPages = steps.count
        dots.currentPageIndicatorTintColor = Palette.text
        dots.pageIndicatorTintColor = Palette.tertiary
        dots.isUserInteractionEnabled = false

        primary.configuration?.baseBackgroundColor = Palette.text
        primary.configuration?.baseForegroundColor = Palette.background
        primary.configuration?.cornerStyle = .capsule
        primary.configuration?.contentInsets = NSDirectionalEdgeInsets(top: 14, leading: 32, bottom: 14, trailing: 32)
        primary.accessibilityIdentifier = "onboarding-next"
        primary.addAction(UIAction { [weak self] _ in self?.advance() }, for: .touchUpInside)

        skip.setTitle("Skip", for: .normal)
        skip.titleLabel?.font = Fonts.ui(.sansMedium, 16)
        skip.tintColor = Palette.secondary
        skip.accessibilityIdentifier = "onboarding-skip"
        skip.addAction(UIAction { [weak self] _ in self?.onFinish?(false) }, for: .touchUpInside)

        demo.setTitle("See a demo", for: .normal)
        demo.titleLabel?.font = Fonts.ui(.sans, 14)
        demo.tintColor = Palette.secondary
        demo.accessibilityIdentifier = "open-demo"
        demo.addAction(UIAction { [weak self] _ in self?.onFinish?(true) }, for: .touchUpInside)

        let footer = UIStackView(arrangedSubviews: [dots, primary, demo])
        footer.axis = .vertical
        footer.alignment = .center
        footer.spacing = 16
        footer.setCustomSpacing(8, after: primary)

        for v in [scroll, pages, footer, skip] as [UIView] { v.translatesAutoresizingMaskIntoConstraints = false }
        scroll.addSubview(pages)
        view.addSubview(scroll)
        view.addSubview(footer)
        view.addSubview(skip)
        let guide = view.safeAreaLayoutGuide
        NSLayoutConstraint.activate([
            skip.topAnchor.constraint(equalTo: guide.topAnchor, constant: 8),
            skip.trailingAnchor.constraint(equalTo: guide.trailingAnchor, constant: -20),
            scroll.topAnchor.constraint(equalTo: skip.bottomAnchor),
            scroll.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            scroll.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            scroll.bottomAnchor.constraint(equalTo: footer.topAnchor, constant: -16),
            pages.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor),
            pages.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor),
            pages.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor),
            pages.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor),
            pages.heightAnchor.constraint(equalTo: scroll.frameLayoutGuide.heightAnchor),
            pages.widthAnchor.constraint(equalTo: scroll.frameLayoutGuide.widthAnchor, multiplier: CGFloat(max(steps.count, 1))),
            footer.leadingAnchor.constraint(equalTo: guide.leadingAnchor, constant: 24),
            footer.trailingAnchor.constraint(equalTo: guide.trailingAnchor, constant: -24),
            footer.bottomAnchor.constraint(equalTo: guide.bottomAnchor, constant: -12),
            primary.widthAnchor.constraint(lessThanOrEqualToConstant: 420),
            primary.widthAnchor.constraint(equalTo: footer.widthAnchor).withPriority(.defaultHigh),
        ])
        render()
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        // Keep the page in view across rotation and the first layout.
        let x = CGFloat(index) * scroll.bounds.width
        if !scroll.isDragging, !scroll.isDecelerating, scroll.contentOffset.x != x {
            scroll.contentOffset.x = x
        }
    }

    private func page(_ step: OnboardingStep) -> UIView {
        let image = UIImageView(image: UIImage(systemName: Self.symbol(step.id), withConfiguration: UIImage.SymbolConfiguration(pointSize: 56, weight: .light)))
        image.tintColor = Palette.text
        image.contentMode = .scaleAspectFit
        let title = UILabel()
        title.text = step.title
        title.font = Fonts.ui(.sansSemibold, 28)
        title.textColor = Palette.text
        title.numberOfLines = 0
        title.textAlignment = .center
        let body = UILabel()
        body.text = step.body
        body.font = Fonts.ui(.sans, 17)
        body.textColor = Palette.secondary
        body.numberOfLines = 0
        body.textAlignment = .center
        let stack = UIStackView(arrangedSubviews: [image, title, body])
        stack.axis = .vertical
        stack.alignment = .center
        stack.spacing = 14
        stack.setCustomSpacing(28, after: image)
        stack.translatesAutoresizingMaskIntoConstraints = false
        let page = UIView()
        page.accessibilityIdentifier = "onboarding-step-\(step.id)"
        page.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.centerYAnchor.constraint(equalTo: page.centerYAnchor),
            stack.leadingAnchor.constraint(equalTo: page.leadingAnchor, constant: 32),
            stack.trailingAnchor.constraint(equalTo: page.trailingAnchor, constant: -32),
            stack.widthAnchor.constraint(lessThanOrEqualToConstant: 480),
        ])
        return page
    }

    private func render() {
        guard !steps.isEmpty else { return }
        dots.currentPage = index
        primary.configuration?.attributedTitle = AttributedString(steps[index].action, attributes: AttributeContainer([.font: Fonts.ui(.sansSemibold, 17)]))
        skip.isHidden = index == steps.count - 1
    }

    private func advance() {
        guard index + 1 < steps.count else { return onFinish?(false) ?? () }
        index += 1
        scroll.setContentOffset(CGPoint(x: CGFloat(index) * scroll.bounds.width, y: 0), animated: true)
        render()
    }

    func scrollViewDidEndDecelerating(_ scrollView: UIScrollView) {
        guard scrollView.bounds.width > 0 else { return }
        index = max(0, min(steps.count - 1, Int((scrollView.contentOffset.x / scrollView.bounds.width).rounded())))
        render()
    }
}

private extension NSLayoutConstraint {
    func withPriority(_ priority: UILayoutPriority) -> NSLayoutConstraint {
        self.priority = priority
        return self
    }
}
