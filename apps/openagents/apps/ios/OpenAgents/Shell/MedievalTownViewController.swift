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
    private let loading = MedievalTownLoadingView()
    private var retry: UIButton { loading.retry }
    private let sprint = UIButton(type: .system)
    private let jump = UIButton(type: .system)
    private let renderQueue = DispatchQueue(label: "com.openagents.medieval-town.render", qos: .userInteractive)
    private var renderer: MedievalTownRenderer?
    private var loadTask: Task<Void, Never>?
    private var loadingTimer: Timer?
    private var displayLink: CADisplayLink?
    private var inFlight = false
    private var visible = false
    private var foreground = true
    private var attached = false
    private var prepared = false
    private var attaching = false
    private var resumeLoading = false
    private var setupHold: DispatchSemaphore?
    private var setupHoldUsed = false
    private var setupRestarts = 0
    private var loadFailed = false
    private var firstFrameShown = false
    private var failureFixtureUsed = false
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
    private static let coldCacheSession = UUID().uuidString
    private lazy var cachePath: String = {
        let arguments = ProcessInfo.processInfo.arguments
        let cold = arguments.contains("-demo") && arguments.contains("-town-cold-cache")
        let name = cold ? "medieval-town-test-\(Self.coldCacheSession)" : "medieval-town"
        return FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask)[0]
            .appendingPathComponent(name, isDirectory: true).path
    }()

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
        loading.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(loading)
        NSLayoutConstraint.activate([
            loading.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            loading.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            loading.topAnchor.constraint(equalTo: view.topAnchor),
            loading.bottomAnchor.constraint(equalTo: view.bottomAnchor),
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
        [close, status, move, sprint, jump].forEach {
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
        setPlayable(false)
        let center = NotificationCenter.default
        observers = [
            center.addObserver(forName: UIApplication.willResignActiveNotification, object: nil, queue: nil) { [weak self] _ in
                MainActor.assumeIsolated {
                    self?.foreground = false
                    self?.updateActive()
                    self?.releaseSetupHold()
                }
            },
            center.addObserver(forName: UIApplication.didEnterBackgroundNotification, object: nil, queue: nil) { [weak self] _ in
                MainActor.assumeIsolated {
                    self?.foreground = false
                    self?.updateActive()
                    self?.releaseSetupHold()
                }
            },
            center.addObserver(forName: UIApplication.didBecomeActiveNotification, object: nil, queue: nil) { [weak self] _ in
                MainActor.assumeIsolated {
                    self?.foreground = true
                    self?.updateActive()
                }
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
        setupHold?.signal()
        displayLink?.invalidate()
        renderer?.cancelLoading()
        loadTask?.cancel()
        loadingTimer?.invalidate()
        observers.forEach(NotificationCenter.default.removeObserver)
        if let town = renderer {
            town.setActive(active: false)
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
        loadFailed = false
        firstFrameShown = false
        loading.reset()
        setPlayable(false)
        retry.isHidden = true
        let arguments = ProcessInfo.processInfo.arguments
        if arguments.contains("-demo"), arguments.contains("-town-load-failure-once"), !failureFixtureUsed {
            failureFixtureUsed = true
            let current = generation
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { [weak self] in self?.showFailure(generation: current) }
            return
        }
        let town = MedievalTownRenderer(cacheDirectory: cachePath)
        renderer = town
        benchmarkSaved = false
        let current = generation
        startLoadingTimer()
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
        guard visible, foreground, prepared, !attached, !attaching, !loadFailed, let town = renderer else { return }
        attaching = true
        view.layoutIfNeeded()
        let size = surface.drawableSize
        let scale = Float(surface.contentScaleFactor)
        let metalLayer = surface.metalLayer
        let pointer = UInt64(UInt(bitPattern: Unmanaged.passUnretained(metalLayer).toOpaque()))
        let queue = renderQueue
        let benchmark = benchmarking
        let current = generation
        // Hold only the demo fixture's first setup, released after deactivation.
        // This exercises the real loader, renderer, and suspension error path.
        let args = ProcessInfo.processInfo.arguments
        if args.contains("-demo"), args.contains("-town-pause-setup"), !setupHoldUsed {
            setupHoldUsed = true
            setupHold = DispatchSemaphore(value: 0)
        }
        let hold = setupHold
        queue.async { [weak self, metalLayer] in
            if let hold { _ = hold.wait(timeout: .now() + 10) }
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
                let suspended: Bool
                if case MedievalTownError.Suspended = error { suspended = true }
                else { suspended = false; logMedievalTownFailure(error, stage: "opening the display") }
                DispatchQueue.main.async { [weak self] in
                    guard let self, current == generation else { return }
                    attaching = false
                    if suspended {
                        setupRestarts += 1
                        prepared = false
                        resumeLoading = true
                        updateActive()
                    } else {
                        showFailure(generation: current)
                    }
                }
            }
        }
    }

    private func stopTown() {
        generation += 1
        renderer?.setActive(active: false)
        releaseSetupHold()
        renderer?.cancelLoading()
        loadTask?.cancel()
        loadTask = nil
        loadingTimer?.invalidate()
        loadingTimer = nil
        displayLink?.invalidate()
        displayLink = nil
        attached = false
        prepared = false
        attaching = false
        resumeLoading = false
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

    private func releaseSetupHold() {
        setupHold?.signal()
        setupHold = nil
    }

    private func showFailure(generation current: Int) {
        guard current == generation else { return }
        loadFailed = true
        loadingTimer?.invalidate()
        loadingTimer = nil
        displayLink?.isPaused = true
        renderer?.setActive(active: false)
        setPlayable(false)
        loading.showFailure(renderer?.loadingProgress(), diagnostics: diagnosticMetrics)
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
        if resumeLoading && visible && foreground {
            resumeLoading = false
            loadTown()
            return
        }
        // Deliver both edges before starting setup, so an old queued activation
        // cannot reopen graphics after a newer background notification.
        renderer?.setActive(active: visible && foreground && !loadFailed)
        if visible && foreground && !firstFrameShown && !loadFailed { startLoadingTimer() }
        else if !visible || !foreground {
            loadingTimer?.invalidate()
            loadingTimer = nil
        }
        attachIfNeeded()
        if visible && foreground && attached { resizeSurface() }
        let active = visible && foreground && attached && !loadFailed
        displayLink?.isPaused = !active
        if !active { resetInput() }
    }

    private func startLoadingTimer() {
        guard loadingTimer == nil, !firstFrameShown, !loadFailed, let town = renderer else { return }
        let current = generation
        let timer = Timer(timeInterval: 0.25, repeats: true) { [weak self] _ in
            guard let self, current == generation, visible, foreground else { return }
            showLoading(town.loadingProgress())
        }
        loadingTimer = timer
        RunLoop.main.add(timer, forMode: .common)
        showLoading(town.loadingProgress())
    }

    private func showLoading(_ progress: MedievalTownLoadingProgress) {
        guard !loadFailed else { return }
        loading.update(progress, diagnostics: diagnosticMetrics)
        if progress.error != nil {
            showFailure(generation: generation)
        } else if progress.finished {
            firstFrameShown = true
            loadingTimer?.invalidate()
            loadingTimer = nil
            setPlayable(true)
        }
    }

    private func setPlayable(_ playable: Bool) {
        loading.isHidden = playable
        [look, move, sprint, jump, status].forEach { $0.isHidden = !playable }
        surface.isAccessibilityElement = playable
        if !playable { resetInput() }
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
        if diagnosticMetrics {
            var snapshot = values
            snapshot["graphicsSetupRestarts"] = setupRestarts
            let data = try? JSONSerialization.data(withJSONObject: snapshot)
            status.accessibilityValue = data.flatMap { String(data: $0, encoding: .utf8) } ?? json
        } else {
            status.accessibilityValue = String(format: "%.0f frames per second", fps)
        }
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

private final class MedievalTownLoadingView: UIView {
    let retry = UIButton(type: .system)
    private let background = CAGradientLayer()
    private let step = UILabel()
    private let asset = UILabel()
    private let details = UILabel()
    private let percent = UILabel()
    private let bar = UIProgressView(progressViewStyle: .default)
    private let error = UILabel()

    override init(frame: CGRect) {
        super.init(frame: frame)
        accessibilityIdentifier = "medieval-town-loading"
        background.colors = [UIColor(red: 0.15, green: 0.23, blue: 0.28, alpha: 1).cgColor,
                             UIColor(red: 0.28, green: 0.32, blue: 0.36, alpha: 1).cgColor]
        background.startPoint = CGPoint(x: 0, y: 0)
        background.endPoint = CGPoint(x: 1, y: 1)
        layer.insertSublayer(background, at: 0)
        let panel = UIVisualEffectView(effect: UIBlurEffect(style: .systemMaterialDark))
        panel.layer.cornerRadius = 24
        panel.clipsToBounds = true
        panel.translatesAutoresizingMaskIntoConstraints = false
        addSubview(panel)
        let title = UILabel()
        title.text = "Medieval Town"
        title.font = .preferredFont(forTextStyle: .title2)
        title.textColor = .white
        title.adjustsFontForContentSizeCategory = true
        title.accessibilityTraits = .header
        title.textAlignment = .center
        [step, asset, details, percent, error].forEach {
            $0.textColor = .white
            $0.textAlignment = .center
            $0.numberOfLines = 0
            $0.adjustsFontForContentSizeCategory = true
            $0.font = .preferredFont(forTextStyle: .subheadline)
        }
        details.font = .preferredFont(forTextStyle: .caption1)
        details.textColor = UIColor.white.withAlphaComponent(0.8)
        percent.font = .monospacedDigitSystemFont(ofSize: 17, weight: .semibold)
        step.accessibilityIdentifier = "medieval-town-loading-stage"
        asset.accessibilityIdentifier = "medieval-town-loading-asset"
        details.accessibilityIdentifier = "medieval-town-loading-details"
        percent.accessibilityIdentifier = "medieval-town-loading-percent"
        bar.accessibilityIdentifier = "medieval-town-loading-progress"
        error.accessibilityIdentifier = "medieval-town-loading-error"
        bar.progressTintColor = UIColor(red: 0.59, green: 0.83, blue: 0.75, alpha: 1)
        bar.trackTintColor = UIColor.white.withAlphaComponent(0.18)
        let stack = UIStackView(arrangedSubviews: [title, step, asset, bar, percent, details, error, retry])
        stack.axis = .vertical
        stack.spacing = 12
        stack.translatesAutoresizingMaskIntoConstraints = false
        let scroll = UIScrollView()
        scroll.translatesAutoresizingMaskIntoConstraints = false
        panel.contentView.addSubview(scroll)
        scroll.addSubview(stack)
        let panelWidth = panel.widthAnchor.constraint(equalToConstant: 360)
        panelWidth.priority = .defaultHigh
        let panelHeight = panel.heightAnchor.constraint(equalTo: stack.heightAnchor, constant: 48)
        panelHeight.priority = .defaultHigh
        NSLayoutConstraint.activate([
            panel.centerXAnchor.constraint(equalTo: safeAreaLayoutGuide.centerXAnchor),
            panel.centerYAnchor.constraint(equalTo: safeAreaLayoutGuide.centerYAnchor),
            panel.widthAnchor.constraint(lessThanOrEqualTo: safeAreaLayoutGuide.widthAnchor, constant: -40),
            panelWidth,
            panelHeight,
            panel.heightAnchor.constraint(lessThanOrEqualTo: safeAreaLayoutGuide.heightAnchor, constant: -24),
            scroll.leadingAnchor.constraint(equalTo: panel.contentView.leadingAnchor),
            scroll.trailingAnchor.constraint(equalTo: panel.contentView.trailingAnchor),
            scroll.topAnchor.constraint(equalTo: panel.contentView.topAnchor),
            scroll.bottomAnchor.constraint(equalTo: panel.contentView.bottomAnchor),
            stack.widthAnchor.constraint(equalTo: scroll.frameLayoutGuide.widthAnchor, constant: -48),
            stack.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor, constant: 24),
            stack.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor, constant: -24),
            stack.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor, constant: 24),
            stack.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor, constant: -24),
            bar.heightAnchor.constraint(equalToConstant: 6),
            retry.heightAnchor.constraint(equalToConstant: 44),
        ])
        reset()
    }

    required init?(coder: NSCoder) { fatalError() }

    override func layoutSubviews() {
        super.layoutSubviews()
        background.frame = bounds
    }

    func reset() {
        step.text = "Checking saved files"
        asset.text = nil
        asset.isHidden = true
        details.text = "Getting the town ready"
        details.accessibilityValue = nil
        percent.text = "0%"
        percent.isHidden = false
        bar.progress = 0
        bar.isHidden = false
        details.isHidden = false
        error.isHidden = true
        retry.isHidden = true
    }

    func update(_ progress: MedievalTownLoadingProgress, diagnostics: Bool) {
        let fraction = progress.fraction.isFinite ? max(0, min(progress.finished ? 1 : 0.99, progress.fraction)) : 0
        let percentage = Int(fraction * 100)
        step.text = progress.stage.isEmpty ? "Getting the town ready" : progress.stage
        asset.text = progress.currentAsset
        asset.isHidden = progress.currentAsset.isEmpty
        percent.text = "\(percentage)%"
        bar.setProgress(fraction, animated: true)
        bar.accessibilityValue = "\(percentage) percent"
        var lines: [String] = []
        if progress.totalBytes > 0 {
            let done = ByteCountFormatter.string(fromByteCount: Int64(clamping: progress.completedBytes), countStyle: .file)
            let total = ByteCountFormatter.string(fromByteCount: Int64(clamping: progress.totalBytes), countStyle: .file)
            lines.append("\(done) of \(total)")
        } else if progress.completedBytes > 0 {
            let done = ByteCountFormatter.string(fromByteCount: Int64(clamping: progress.completedBytes), countStyle: .file)
            lines.append("\(done) received")
        }
        if progress.totalFiles > 0 { lines.append("\(progress.completedFiles) of \(progress.totalFiles) files") }
        details.text = lines.isEmpty ? "Getting the town ready" : lines.joined(separator: " · ")
        if diagnostics {
            let values: [String: Any] = ["stage": progress.stage, "fraction": fraction,
                "completedBytes": progress.completedBytes, "totalBytes": progress.totalBytes,
                "completedFiles": progress.completedFiles, "totalFiles": progress.totalFiles,
                "currentAsset": progress.currentAsset, "finished": progress.finished,
                "failed": progress.error != nil]
            if let data = try? JSONSerialization.data(withJSONObject: values, options: [.sortedKeys]) {
                details.accessibilityValue = String(data: data, encoding: .utf8)
            }
        } else { details.accessibilityValue = nil }
    }

    func showFailure(_ progress: MedievalTownLoadingProgress?, diagnostics: Bool) {
        if let progress { update(progress, diagnostics: diagnostics) }
        step.text = "Loading stopped"
        error.text = progress?.error ?? "Couldn't open the town. Try again."
        percent.isHidden = true
        bar.isHidden = true
        error.isHidden = false
        retry.isHidden = false
    }
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
