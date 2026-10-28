import UIKit
import Metal
import QuartzCore

private final class Surface: UIView {
    override class var layerClass: AnyClass { CAMetalLayer.self }
    var metalLayer: CAMetalLayer { layer as! CAMetalLayer }
    override var canBecomeFirstResponder: Bool { false }
}

/// Native lifecycle/input adapter. Coax owns the screen and all touch policy.
final class HomeViewController: UIViewController {
    private var surface: Surface { view as! Surface }
    private var renderer: HomeRenderer?
    private var clock: CADisplayLink?
    private var handle: Int64 = 0
    private weak var trackedTouch: UITouch?
    private var startElement: UIAccessibilityElement?
    private var lastSize = CGSize.zero
    private var invocationCount: Int64 = 0
    private let wallet = SparkWallet()
    private let balanceLabel = UILabel()
    private var walletTask: Task<Void, Never>?
    override var prefersStatusBarHidden: Bool { true }
    override var prefersHomeIndicatorAutoHidden: Bool { true }
    override func loadView() { view = Surface(); view.backgroundColor = .black; view.isMultipleTouchEnabled = false }
    override func viewDidLoad() {
        super.viewDidLoad()
        do {
            guard let url = Bundle.main.url(forResource: "Home", withExtension: "pb") else { throw HomeError.resource("Home.pb") }
            let bytes = try Data(contentsOf: url)
            handle = bytes.withUnsafeBytes { mr_create($0.bindMemory(to: UInt8.self).baseAddress, $0.count, Int(metareal_home_capacity())) }
            guard handle != 0, metareal_home_init(handle), mr_ok(handle) else { throw HomeError.resource("Home configuration") }
            renderer = try HomeRenderer(layer: surface.metalLayer)
            let element = HomeActionElement(accessibilityContainer: surface)
            element.accessibilityLabel = "Start"
            element.accessibilityTraits = .button
            element.accessibilityIdentifier = "home.start"
            element.accessibilityValue = "0"
            element.activate = { [weak self] in self?.activateAccessibleButton() ?? false }
            startElement = element
            setUpBalance()
            view.accessibilityElements = [balanceLabel, element]
            NotificationCenter.default.addObserver(self, selector: #selector(suspend), name: UIApplication.willResignActiveNotification, object: nil)
            NotificationCenter.default.addObserver(self, selector: #selector(resume), name: UIApplication.didBecomeActiveNotification, object: nil)
            NotificationCenter.default.addObserver(self, selector: #selector(motionChanged), name: UIAccessibility.reduceMotionStatusDidChangeNotification, object: nil)
            resume()
        } catch {
            // Fail visibly; never leave a blank view after a shader/resource error.
            let message = UILabel(); message.text = "Home unavailable\n\(error.localizedDescription)"
            message.numberOfLines = 0; message.textColor = .white; message.textAlignment = .center
            message.frame = view.bounds; message.autoresizingMask = [.flexibleWidth, .flexibleHeight]
            view.addSubview(message)
            NSLog("OpenAgents home: %@", String(describing: error))
        }
    }
    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        let scale = view.window?.screen.scale ?? UIScreen.main.scale
        let size = CGSize(width: view.bounds.width * scale, height: view.bounds.height * scale)
        surface.metalLayer.contentsScale = scale
        if lastSize != size {
            surface.metalLayer.drawableSize = size
            renderer?.resize(size)
            lastSize = size
            metareal_home_resume(handle)
            trackedTouch = nil
            updateScene(at: CACurrentMediaTime())
        }
    }
    @objc private func suspend() {
        clock?.invalidate(); clock = nil; trackedTouch = nil
        metareal_home_resume(handle)
        walletTask?.cancel(); walletTask = nil
        Task { [wallet] in await wallet.disconnect() }
    }
    @objc private func resume() {
        refreshBalance()
        guard renderer != nil, clock == nil else { return }
        metareal_home_resume(handle)
        let link = CADisplayLink(target: DisplayTarget(owner: self), selector: #selector(DisplayTarget.tick(_:)))
        link.preferredFrameRateRange = CAFrameRateRange(minimum: 30, maximum: 120, preferred: 60)
        link.add(to: .main, forMode: .common); clock = link
    }
    private func setUpBalance() {
        balanceLabel.translatesAutoresizingMaskIntoConstraints = false
        balanceLabel.textColor = UIColor(white: 1, alpha: 0.85)
        balanceLabel.font = UIFont(name: "Michroma", size: 17) ?? .monospacedDigitSystemFont(ofSize: 17, weight: .regular)
        balanceLabel.textAlignment = .center
        balanceLabel.numberOfLines = 0
        balanceLabel.accessibilityIdentifier = "wallet.balance"
        view.addSubview(balanceLabel)
        NSLayoutConstraint.activate([
            balanceLabel.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 24),
            balanceLabel.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor, constant: 16),
            balanceLabel.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor, constant: -16),
        ])
        show(.opening)
    }
    private func refreshBalance() {
        guard walletTask == nil, balanceLabel.superview != nil else { return }
        walletTask = Task { [weak self, wallet] in
            let state = await wallet.balance()
            await MainActor.run {
                guard let self, !Task.isCancelled else { return }
                self.show(state); self.walletTask = nil
            }
        }
    }
    private func show(_ state: SparkWallet.State) {
        switch state {
        case .opening: balanceLabel.text = "Opening your wallet…"
        case .balance(let sats):
            let number = NumberFormatter(); number.numberStyle = .decimal
            balanceLabel.text = "₿" + (number.string(from: NSNumber(value: sats)) ?? String(sats))
            NSLog("OpenAgents wallet: balance shown (%llu sats)", sats)
        case .noWallet: balanceLabel.text = "No wallet on this phone yet."
        case .locked: balanceLabel.text = "Your wallet is locked. Unlock your phone, then reopen OpenAgents."
        case .unreadable: balanceLabel.text = "The wallet saved on this phone can't be read. It has been left as it is."
        case .failed(let message): balanceLabel.text = message
        }
    }
    @objc private func motionChanged() { metareal_home_resume(handle) }
    fileprivate func frame(_ link: CADisplayLink) { updateScene(at: link.timestamp) }
    private func updateScene(at timestamp: Double) {
        guard renderer != nil, lastSize.width > 0, lastSize.height > 0 else { return }
        let scale = surface.metalLayer.contentsScale
        mr_clear_frame(handle)
        metareal_home_frame(handle, timestamp, lastSize.width, lastSize.height, scale, view.safeAreaInsets.bottom * scale, UIAccessibility.isReduceMotionEnabled)
        guard mr_ok(handle) else { suspend(); NSLog("OpenAgents: invalid home draw packet"); return }
        renderer?.draw(handle: handle, time: timestamp)
        if let bar = mr_bar(handle)?.pointee {
            let rect = CGRect(x: CGFloat(bar.x)/scale, y: CGFloat(bar.y)/scale, width: CGFloat(bar.width)/scale, height: CGFloat(bar.height)/scale)
            startElement?.accessibilityFrameInContainerSpace = rect
        }
    }
    private func route(_ phase: Int64, _ touch: UITouch) {
        let point = touch.location(in: view); let scale = surface.metalLayer.contentsScale
        let activated = metareal_home_touch(handle, phase, point.x * scale, point.y * scale)
        if activated { publishInvocation() }
    }
    private func publishInvocation() {
        invocationCount = metareal_home_presses(handle)
        startElement?.accessibilityValue = String(invocationCount)
        if let bytes = mr_output(handle), mr_output_size(handle) > 0 {
            let invocation = Data(bytes: bytes, count: mr_output_size(handle))
            // Bytes remain a real HudInvocation at the routing boundary.
            NotificationCenter.default.post(name: .init("MetarealHudInvocation"), object: invocation)
            NSLog("Metareal home.start #%lld (%ld protobuf bytes)", invocationCount, invocation.count)
        }
        UIImpactFeedbackGenerator(style: .light).impactOccurred()
    }
    private func activateAccessibleButton() -> Bool {
        guard let bar = mr_bar(handle)?.pointee, bar.width > 0 else { return false }
        let x = Double(bar.x+bar.width/2), y = Double(bar.y+bar.height/2)
        _ = metareal_home_touch(handle, 0, x, y)
        if metareal_home_touch(handle, 2, x, y) { publishInvocation(); return true }
        return false
    }
    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard trackedTouch == nil, let touch = touches.first else { return }
        trackedTouch = touch; route(0, touch)
    }
    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        if let touch = trackedTouch, touches.contains(touch) { route(1, touch) }
    }
    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        if let touch = trackedTouch, touches.contains(touch) { route(2, touch); trackedTouch = nil }
    }
    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        if let touch = trackedTouch { route(3, touch) }; trackedTouch = nil
    }
    deinit { clock?.invalidate(); NotificationCenter.default.removeObserver(self); mr_destroy(handle) }
}
private final class HomeActionElement: UIAccessibilityElement {
    var activate: (() -> Bool)?
    override func accessibilityActivate() -> Bool { activate?() ?? false }
}
enum HomeError: LocalizedError {
    case resource(String)
    var errorDescription: String? { if case let .resource(name) = self { return name }; return nil }
}

private final class DisplayTarget: NSObject {
    weak var owner: HomeViewController?
    init(owner: HomeViewController) { self.owner = owner }
    @objc func tick(_ link: CADisplayLink) { owner?.frame(link) }
}
