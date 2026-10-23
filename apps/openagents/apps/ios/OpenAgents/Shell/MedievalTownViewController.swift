import QuartzCore
import UIKit

/// A native Metal surface. Rust loads the town, moves the player, and draws
/// each frame; UIKit supplies the surface and forwards touch input.
final class MedievalTownViewController: UIViewController {
    private let surface = MedievalTownMetalView()
    private let move = MedievalTownMoveControl()
    private let look = UIView()
    private let status = UILabel()
    private var statusWidth: NSLayoutConstraint?
    private var statusHeight: NSLayoutConstraint?
    private let retry = UIButton(type: .system)
    private let sprint = UIButton(type: .system)
    private let jump = UIButton(type: .system)
    private let renderQueue = DispatchQueue(label: "com.openagents.medieval-town.render", qos: .userInteractive)
    private var renderer: MedievalTownRenderer?
    private var loadTask: Task<Void, Never>?
    private var displayLink: CADisplayLink?
    private var inFlight = false
    private var visible = false
    private var foreground = true
    private var attached = false
    private var prepared = false
    private var attaching = false
    private var generation = 0
    private var lastMetricsTime: CFTimeInterval = 0
    private var lastSize = CGSize.zero
    private var lookDelta = CGPoint.zero
    private var running = false
    private var jumping = false
    private var observers: [NSObjectProtocol] = []
    private let benchmarking = ProcessInfo.processInfo.arguments.contains("-town-bench")
    private let diagnosticMetrics = ProcessInfo.processInfo.arguments.contains("-demo")
        || ProcessInfo.processInfo.arguments.contains("-town-bench")
        || ProcessInfo.processInfo.arguments.contains("-town-metrics")
    private var benchmarkSaved = false

    override var prefersStatusBarHidden: Bool { true }
    override var prefersHomeIndicatorAutoHidden: Bool { true }
    override var preferredScreenEdgesDeferringSystemGestures: UIRectEdge { [.bottom, .left, .right] }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        surface.accessibilityIdentifier = "medieval-town-surface"
        surface.accessibilityLabel = "Medieval Town"
        surface.isAccessibilityElement = true
        look.accessibilityIdentifier = "medieval-town-look"
        look.accessibilityLabel = "Look around"
        look.isAccessibilityElement = true
        let pan = UIPanGestureRecognizer(target: self, action: #selector(dragLook(_:)))
        pan.maximumNumberOfTouches = 1
        look.addGestureRecognizer(pan)
        [surface, look].forEach {
            $0.translatesAutoresizingMaskIntoConstraints = false
            view.addSubview($0)
        }
        NSLayoutConstraint.activate([
            surface.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            surface.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            surface.topAnchor.constraint(equalTo: view.topAnchor),
            surface.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            look.leadingAnchor.constraint(equalTo: view.centerXAnchor),
            look.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            look.topAnchor.constraint(equalTo: view.topAnchor),
            look.bottomAnchor.constraint(equalTo: view.bottomAnchor),
        ])

        let close = UIButton(type: .system)
        close.setImage(UIImage(systemName: "xmark"), for: .normal)
        close.accessibilityLabel = "Close Medieval Town"
        close.accessibilityIdentifier = "medieval-town-close"
        close.addAction(UIAction { [weak self] _ in self?.dismiss(animated: true) }, for: .touchUpInside)
        configureButton(close)
        status.text = "Opening Medieval Town…"
        status.textColor = .white
        status.font = .monospacedDigitSystemFont(ofSize: 11, weight: .medium)
        status.textAlignment = .center
        status.numberOfLines = 2
        status.backgroundColor = UIColor.black.withAlphaComponent(0.55)
        status.layer.cornerRadius = 12
        status.clipsToBounds = true
        status.isAccessibilityElement = true
        status.accessibilityIdentifier = "medieval-town-metrics"
        retry.setTitle("Try again", for: .normal)
        retry.accessibilityIdentifier = "medieval-town-retry"
        retry.addAction(UIAction { [weak self] _ in self?.loadTown() }, for: .touchUpInside)
        configureButton(retry)
        retry.isHidden = true
        sprint.setTitle("Run", for: .normal)
        sprint.accessibilityIdentifier = "medieval-town-run"
        sprint.addAction(UIAction { [weak self] _ in
            guard let self else { return }
            running.toggle()
            sprint.isSelected = running
            sprint.accessibilityValue = running ? "On" : "Off"
            sprint.backgroundColor = (running ? UIColor.systemBlue : .black).withAlphaComponent(0.55)
        }, for: .touchUpInside)
        configureButton(sprint)
        jump.setImage(UIImage(systemName: "arrow.up"), for: .normal)
        jump.accessibilityLabel = "Jump"
        jump.accessibilityIdentifier = "medieval-town-jump"
        jump.addAction(UIAction { [weak self] _ in self?.jumping = true }, for: .touchDown)
        jump.addAction(UIAction { [weak self] _ in self?.jumping = false }, for: [.touchUpInside, .touchUpOutside, .touchCancel])
        configureButton(jump)
        move.accessibilityIdentifier = "medieval-town-move"
        move.accessibilityLabel = "Move"
        [close, status, retry, move, sprint, jump].forEach {
            $0.translatesAutoresizingMaskIntoConstraints = false
            view.addSubview($0)
        }
        let safe = view.safeAreaLayoutGuide
        let statusWidth = status.widthAnchor.constraint(equalToConstant: 160)
        let statusHeight = status.heightAnchor.constraint(equalToConstant: 40)
        self.statusWidth = statusWidth
        self.statusHeight = statusHeight
        NSLayoutConstraint.activate([
            close.leadingAnchor.constraint(equalTo: safe.leadingAnchor, constant: 12),
            close.topAnchor.constraint(equalTo: safe.topAnchor, constant: 8),
            close.widthAnchor.constraint(equalToConstant: 44),
            close.heightAnchor.constraint(equalToConstant: 44),
            status.trailingAnchor.constraint(equalTo: safe.trailingAnchor, constant: -14),
            status.topAnchor.constraint(equalTo: safe.topAnchor, constant: 10),
            statusWidth,
            statusHeight,
            retry.centerXAnchor.constraint(equalTo: safe.centerXAnchor),
            retry.topAnchor.constraint(equalTo: status.bottomAnchor, constant: 10),
            retry.widthAnchor.constraint(equalToConstant: 120),
            retry.heightAnchor.constraint(equalToConstant: 44),
            move.leadingAnchor.constraint(equalTo: safe.leadingAnchor, constant: 22),
            move.bottomAnchor.constraint(equalTo: safe.bottomAnchor, constant: -22),
            move.widthAnchor.constraint(equalToConstant: 124),
            move.heightAnchor.constraint(equalToConstant: 124),
            jump.trailingAnchor.constraint(equalTo: safe.trailingAnchor, constant: -22),
            jump.bottomAnchor.constraint(equalTo: safe.bottomAnchor, constant: -26),
            jump.widthAnchor.constraint(equalToConstant: 56),
            jump.heightAnchor.constraint(equalToConstant: 56),
            sprint.trailingAnchor.constraint(equalTo: jump.leadingAnchor, constant: -12),
            sprint.centerYAnchor.constraint(equalTo: jump.centerYAnchor),
            sprint.widthAnchor.constraint(equalToConstant: 58),
            sprint.heightAnchor.constraint(equalToConstant: 44),
        ])
        let center = NotificationCenter.default
        observers = [
            center.addObserver(forName: UIApplication.willResignActiveNotification, object: nil, queue: .main) { [weak self] _ in
                self?.foreground = false
                self?.updateActive()
            },
            center.addObserver(forName: UIApplication.didEnterBackgroundNotification, object: nil, queue: .main) { [weak self] _ in
                self?.foreground = false
                self?.updateActive()
            },
            center.addObserver(forName: UIApplication.didBecomeActiveNotification, object: nil, queue: .main) { [weak self] _ in
                self?.foreground = true
                self?.updateActive()
            },
        ]
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        visible = true
        foreground = UIApplication.shared.applicationState == .active
        if renderer == nil { loadTown() }
        else { updateActive() }
    }

    override func viewDidDisappear(_ animated: Bool) {
        super.viewDidDisappear(animated)
        visible = false
        if isBeingDismissed || presentingViewController?.isBeingDismissed == true {
            stopTown()
        } else {
            updateActive()
        }
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        resizeSurface()
    }

    deinit {
        displayLink?.invalidate()
        loadTask?.cancel()
        observers.forEach(NotificationCenter.default.removeObserver)
        if let town = renderer {
            let layer = surface.metalLayer
            renderQueue.async {
                town.setActive(active: false)
                town.detach()
                _ = layer
            }
        }
    }

    private func configureButton(_ button: UIButton) {
        button.tintColor = .white
        button.setTitleColor(.white, for: .normal)
        button.titleLabel?.font = .systemFont(ofSize: 14, weight: .semibold)
        button.backgroundColor = UIColor.black.withAlphaComponent(0.55)
        button.layer.cornerRadius = 16
    }

    private func loadTown() {
        guard visible else { return }
        stopTown()
        statusWidth?.constant = 160
        statusHeight?.constant = 40
        status.text = "Opening Medieval Town…"
        status.accessibilityValue = "Opening Medieval Town."
        retry.isHidden = true
        let path = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("medieval-town", isDirectory: true).path
        let town = MedievalTownRenderer(cacheDirectory: path)
        renderer = town
        benchmarkSaved = false
        let current = generation
        loadTask = Task { [weak self] in
            do {
                try await town.prepare()
                guard !Task.isCancelled, let self, current == generation else { return }
                prepared = true
                updateActive()
            } catch {
                guard !Task.isCancelled else { return }
                logMedievalTownFailure(error, stage: "loading")
                self?.showFailure(generation: current)
            }
        }
    }

    private func attachIfNeeded() {
        guard visible, foreground, prepared, !attached, !attaching, let town = renderer else { return }
        attaching = true
        view.layoutIfNeeded()
        let size = surface.drawableSize
        let scale = Float(surface.contentScaleFactor)
        let metalLayer = surface.metalLayer
        let pointer = UInt64(UInt(bitPattern: Unmanaged.passUnretained(metalLayer).toOpaque()))
        let queue = renderQueue
        let benchmark = benchmarking
        let current = generation
        queue.async { [weak self, metalLayer] in
            do {
                try town.attach(layer: pointer, width: UInt32(size.width), height: UInt32(size.height), scale: scale)
                if benchmark { town.beginBenchmark() }
                _ = metalLayer
                DispatchQueue.main.async { [weak self] in
                    guard let self, current == generation else {
                        queue.async { town.detach() }
                        return
                    }
                    attaching = false
                    attached = true
                    lastSize = size
                    startDisplayLink()
                    updateActive()
                }
            } catch {
                logMedievalTownFailure(error, stage: "opening the display")
                DispatchQueue.main.async { [weak self] in
                    guard let self, current == generation else { return }
                    attaching = false
                    showFailure(generation: current)
                }
            }
        }
    }

    private func stopTown() {
        generation += 1
        loadTask?.cancel()
        loadTask = nil
        displayLink?.invalidate()
        displayLink = nil
        attached = false
        prepared = false
        attaching = false
        inFlight = false
        resetInput()
        if let town = renderer {
            // The queued detach runs after any submitted frame, before the
            // layer can disappear with this controller.
            let layer = surface.metalLayer
            renderQueue.async {
                town.setActive(active: false)
                town.detach()
                _ = layer
            }
        }
        renderer = nil
    }

    private func showFailure(generation current: Int) {
        guard current == generation else { return }
        displayLink?.isPaused = true
        if let town = renderer { renderQueue.async { town.setActive(active: false) } }
        statusWidth?.constant = 160
        statusHeight?.constant = 40
        status.text = "Couldn't open the town."
        status.accessibilityValue = "Couldn't open the town."
        retry.isHidden = false
    }

    private func startDisplayLink() {
        let proxy = MedievalTownDisplayLinkTarget(owner: self)
        let link = CADisplayLink(target: proxy, selector: #selector(MedievalTownDisplayLinkTarget.tick(_:)))
        link.preferredFrameRateRange = CAFrameRateRange(minimum: 60, maximum: 60, preferred: 60)
        link.add(to: .main, forMode: .common)
        displayLink = link
    }

    private func updateActive() {
        attachIfNeeded()
        if visible && foreground && attached { resizeSurface() }
        let active = visible && foreground && attached
        displayLink?.isPaused = !active
        if !active { resetInput() }
        if let town = renderer { renderQueue.async { town.setActive(active: active) } }
    }

    private func resizeSurface() {
        surface.contentScaleFactor = view.window?.screen.scale ?? traitCollection.displayScale
        let size = surface.drawableSize
        // Rust chooses the render resolution for the current screen.
        guard visible, foreground, attached, size != lastSize, let town = renderer else { return }
        lastSize = size
        let scale = Float(surface.contentScaleFactor)
        let current = generation
        renderQueue.async { [weak self] in
            do { try town.resize(width: UInt32(size.width), height: UInt32(size.height), scale: scale) }
            catch {
                logMedievalTownFailure(error, stage: "resizing the display")
                DispatchQueue.main.async { [weak self] in self?.showFailure(generation: current) }
            }
        }
    }

    fileprivate func draw(_ link: CADisplayLink) {
        guard !inFlight, visible, foreground, attached, let town = renderer else { return }
        inFlight = true
        let current = generation
        let input = move.value
        let delta = lookDelta
        lookDelta = .zero
        let run = running
        let jump = jumping
        let timestamp = link.timestamp
        let report = timestamp - lastMetricsTime >= 0.25
        if report { lastMetricsTime = timestamp }
        renderQueue.async { [weak self] in
            do {
                try town.frame(timestamp: timestamp, moveX: Float(input.x), moveZ: Float(input.y), lookX: Float(delta.x), lookY: Float(delta.y), run: run, jump: jump)
                let metrics = report ? town.metrics() : nil
                DispatchQueue.main.async { [weak self] in
                    guard let self, current == generation else { return }
                    inFlight = false
                    if let metrics { showMetrics(metrics) }
                }
            } catch {
                logMedievalTownFailure(error, stage: "drawing a frame")
                DispatchQueue.main.async { [weak self] in
                    guard let self, current == generation else { return }
                    inFlight = false
                    showFailure(generation: current)
                }
            }
        }
    }

    private func showMetrics(_ json: String) {
        guard let data = json.data(using: .utf8),
              let values = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else { return }
        let fpsKey = benchmarking ? "fps" : "liveFps"
        let fps = (values[fpsKey] as? NSNumber)?.doubleValue ?? 0
        statusWidth?.constant = 74
        statusHeight?.constant = 28
        status.text = String(format: "%.0f FPS", fps)
        status.accessibilityValue = diagnosticMetrics ? json : String(format: "%.0f frames per second", fps)
        if benchmarking, values["benchmarkComplete"] as? Bool == true, !benchmarkSaved {
            benchmarkSaved = true
            displayLink?.isPaused = true
            statusWidth?.constant = 176
            status.text = String(format: "%.1f FPS · Test complete", fps)
            let destination = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
                .appendingPathComponent("medieval-town-benchmark.json")
            renderQueue.async {
                do { try Data(json.utf8).write(to: destination, options: .atomic) }
                catch { logMedievalTownFailure(error, stage: "saving the frame measurement") }
            }
        }
    }

    @objc private func dragLook(_ gesture: UIPanGestureRecognizer) {
        let delta = gesture.translation(in: look)
        lookDelta.x += delta.x
        lookDelta.y += delta.y
        gesture.setTranslation(.zero, in: look)
    }

    private func resetInput() {
        move.reset()
        lookDelta = .zero
        jumping = false
    }
}

private func logMedievalTownFailure(_ error: Error, stage: String) {
    NSLog("Medieval Town %@ failed: %@", stage, String(error.localizedDescription.prefix(512)))
}

private final class MedievalTownMetalView: UIView {
    override class var layerClass: AnyClass { CAMetalLayer.self }
    var metalLayer: CAMetalLayer { layer as! CAMetalLayer }
    var drawableSize: CGSize {
        CGSize(width: max(1, (bounds.width * contentScaleFactor).rounded()), height: max(1, (bounds.height * contentScaleFactor).rounded()))
    }
}

private final class MedievalTownDisplayLinkTarget: NSObject {
    weak var owner: MedievalTownViewController?
    init(owner: MedievalTownViewController) { self.owner = owner }
    @objc func tick(_ link: CADisplayLink) { owner?.draw(link) }
}

/// Maps a finger's displacement to input axes; movement stays in Rust.
private final class MedievalTownMoveControl: UIControl {
    private let thumb = UIView()
    private(set) var value = CGPoint.zero

    override init(frame: CGRect) {
        super.init(frame: frame)
        backgroundColor = UIColor.black.withAlphaComponent(0.32)
        layer.borderWidth = 1
        layer.borderColor = UIColor.white.withAlphaComponent(0.3).cgColor
        thumb.backgroundColor = UIColor.white.withAlphaComponent(0.6)
        thumb.isUserInteractionEnabled = false
        addSubview(thumb)
        isAccessibilityElement = true
        accessibilityTraits = .allowsDirectInteraction
    }

    required init?(coder: NSCoder) { fatalError() }

    override func layoutSubviews() {
        super.layoutSubviews()
        layer.cornerRadius = bounds.width / 2
        thumb.bounds = CGRect(x: 0, y: 0, width: 46, height: 46)
        thumb.layer.cornerRadius = 23
        placeThumb()
    }

    override func beginTracking(_ touch: UITouch, with event: UIEvent?) -> Bool {
        update(touch.location(in: self))
        return true
    }

    override func continueTracking(_ touch: UITouch, with event: UIEvent?) -> Bool {
        update(touch.location(in: self))
        return true
    }

    override func endTracking(_ touch: UITouch?, with event: UIEvent?) { reset() }
    override func cancelTracking(with event: UIEvent?) { reset() }

    func reset() {
        value = .zero
        placeThumb()
    }

    private func update(_ point: CGPoint) {
        let radius = max(1, (bounds.width - thumb.bounds.width) / 2)
        let x = (point.x - bounds.midX) / radius
        let y = (bounds.midY - point.y) / radius
        let length = max(1, sqrt(x * x + y * y))
        value = CGPoint(x: x / length, y: y / length)
        placeThumb()
    }

    private func placeThumb() {
        let radius = max(1, (bounds.width - thumb.bounds.width) / 2)
        thumb.center = CGPoint(x: bounds.midX + value.x * radius, y: bounds.midY - value.y * radius)
    }
}
