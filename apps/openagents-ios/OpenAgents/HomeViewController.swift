import UIKit
import Metal
import QuartzCore

private final class Surface: UIView {
    override class var layerClass: AnyClass { CAMetalLayer.self }
    var metalLayer: CAMetalLayer { layer as! CAMetalLayer }
    override var canBecomeFirstResponder: Bool { false }
}

/// Native lifecycle/input adapter. Coax owns the screen and all touch policy.
/// Start opens the Grid: metareal/grid.coax draws it and reads the fingers,
/// metareal/player.coax walks it, metareal/cast.coax animates you and the
/// robots, and this file passes numbers between them.
final class HomeViewController: UIViewController {
    private var surface: Surface { view as! Surface }
    private var house: HauntViewController?
    private let housePortal = UIButton(type: .custom)
    private var renderer: HomeRenderer?
    private var clock: CADisplayLink?
    private var handle: Int64 = 0
    private weak var trackedTouch: UITouch?
    private var startElement: UIAccessibilityElement?
    private var lastSize = CGSize.zero
    private var invocationCount: Int64 = 0
    private let wallet = SparkHost()
    private let balanceLabel = UILabel()
    private let walletButton = UIButton(type: .custom)
    private var walletTask: Task<Void, Never>?
    /// Voice: the toggle, level and caption (Voice/VoiceBar.swift).
    private let voice = VoiceBar()
    private var gridHandle: Int64 = 0
    private var player = metareal_player_place(0, 0, 0, 0)
    private var sticks = metareal_sticks_reset()
    private var gridTouches: [ObjectIdentifier: Int64] = [:]
    private var walls: [Float] = []
    private var danceElement: UIAccessibilityElement?
    private var danceRequested = false
    private var nextTouchSlot: Int64 = 0
    private var lastTick: Double?
    private var gridElement: UIAccessibilityElement?
    private var youElement: UIAccessibilityElement?
    // Cast.bin, admitted by metareal_cast_open; its bytes stay put for every call.
    private var castBytes: UnsafeMutableRawBufferPointer?
    private var cast: CastAssets?
    private var anim = coax_t_Anim(move: 0, start: 0, from: 0, from_start: 0, fade_start: -1, air: false, dance: false)
    private var gridStart: Double?
    // This frame's palette (the robots posed before you move) and the robots'
    // body capsules from it, which you collide with.
    private var palette: [coax_t_Joint] = []
    private var bodies: [Float] = []
    private var inGrid: Bool { gridHandle != 0 }
    /// The home is the Grid seen from high above (metareal/camera.coax's
    /// overview) under the Start bar; Start dollies the camera down to you,
    /// and then you play.
    private enum Stage { case overview, arriving(coax_t_Dolly), playing }
    private var stage = Stage.overview
    private var playing: Bool { if case .playing = stage { return true }; return false }
    private var overviewEpoch: Double?
    private var lastShot: coax_t_Shot?
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
            setUpVoice()
            setUpGrid()
            // The overview and the dolly down play once, on the first launch;
            // after that the app opens on the camera behind you.
            let seen = UserDefaults.standard.bool(forKey: "OAGridSeen")
            housePortal.setTitle("Haunted House", for: .normal)
            housePortal.titleLabel?.font = UIFont(name: "Michroma", size: 10)
            housePortal.setTitleColor(.white, for: .normal)
            housePortal.accessibilityIdentifier = "grid.haunt"
            housePortal.accessibilityLabel = "Haunted House portal"
            housePortal.isHidden = true
            housePortal.addAction(UIAction { [weak self] _ in self?.enterHouse() }, for: .touchUpInside)
            view.addSubview(housePortal)
            view.accessibilityElements = [element]
            if seen, inGrid { arrive() }
            NotificationCenter.default.addObserver(self, selector: #selector(suspend), name: UIApplication.willResignActiveNotification, object: nil)
            NotificationCenter.default.addObserver(self, selector: #selector(background), name: UIApplication.didEnterBackgroundNotification, object: nil)
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
            gridElement?.accessibilityFrameInContainerSpace = view.bounds
            metareal_home_resume(handle)
            trackedTouch = nil
            updateScene(at: CACurrentMediaTime())
        }
    }
    @objc private func suspend() {
        clock?.invalidate(); clock = nil; trackedTouch = nil
        metareal_home_resume(handle)
        sticks = metareal_sticks_reset(); gridTouches = [:]; lastTick = nil
    }
    // Face ID and the camera prompt resign active; only leaving the app
    // closes the wallet (receiving still works: the SSP holds Lightning).
    @objc private func background() {
        guard presentedViewController == nil else { return }
        walletTask?.cancel(); walletTask = nil
        Task { [wallet] in await wallet.disconnect() }
        voice.stop()
    }
    @objc private func resume() {
        refreshBalance()
        guard house == nil, renderer != nil, clock == nil else { return }
        metareal_home_resume(handle)
        let link = CADisplayLink(target: DisplayTarget(owner: self), selector: #selector(DisplayTarget.tick(_:)))
        link.preferredFrameRateRange = CAFrameRateRange(minimum: 30, maximum: 120, preferred: 60)
        link.add(to: .main, forMode: .common); clock = link
    }
    private func setUpBalance() {
        balanceLabel.translatesAutoresizingMaskIntoConstraints = false
        balanceLabel.textColor = UIColor(white: 1, alpha: 0.85)
        balanceLabel.font = UIFont(name: "Michroma", size: 12) ?? .monospacedDigitSystemFont(ofSize: 12, weight: .regular)
        balanceLabel.textAlignment = .right
        // Shown on the Grid once you play, not over the home's overview.
        balanceLabel.isHidden = true; walletButton.isHidden = true
        balanceLabel.numberOfLines = 1
        balanceLabel.accessibilityIdentifier = "wallet.balance"
        view.addSubview(balanceLabel)
        // The balance opens the wallet once there is one.
        walletButton.translatesAutoresizingMaskIntoConstraints = false
        walletButton.accessibilityIdentifier = "wallet.open"
        walletButton.accessibilityLabel = "Wallet"
        walletButton.isEnabled = false
        walletButton.addAction(UIAction { [weak self] _ in self?.openWallet() }, for: .touchUpInside)
        view.addSubview(walletButton)
        NSLayoutConstraint.activate([
            walletButton.topAnchor.constraint(equalTo: balanceLabel.topAnchor, constant: -14),
            walletButton.bottomAnchor.constraint(equalTo: balanceLabel.bottomAnchor, constant: 14),
            walletButton.leadingAnchor.constraint(equalTo: balanceLabel.leadingAnchor, constant: -14),
            walletButton.trailingAnchor.constraint(equalTo: balanceLabel.trailingAnchor, constant: 14),
            balanceLabel.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 14),
            balanceLabel.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor, constant: -16),
            balanceLabel.widthAnchor.constraint(lessThanOrEqualTo: view.widthAnchor, multiplier: 0.6),
        ])
        show(.opening)
    }
    private func setUpVoice() {
        voice.translatesAutoresizingMaskIntoConstraints = false
        voice.presenter = self
        // Shown on the Grid once you play, like the balance.
        voice.isHidden = true
        view.addSubview(voice)
        NSLayoutConstraint.activate([
            voice.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 0),
            voice.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor, constant: 16),
            voice.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor, constant: -16),
            voice.heightAnchor.constraint(equalToConstant: 170),
        ])
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
    private func openWallet() {
        let screen = WalletViewController(host: wallet)
        // The wallet closed: show the balance it may have changed.
        screen.closed = { [weak self] in self?.refreshBalance() }
        let sheet = UINavigationController(rootViewController: screen)
        sheet.overrideUserInterfaceStyle = .dark
        sheet.view.tintColor = .white
        present(sheet, animated: true)
    }
    private func show(_ state: SparkHost.State) {
        if case .balance = state { walletButton.isEnabled = true } else { walletButton.isEnabled = false }
        // The corner stays small: a figure, or a dash with the reason spoken.
        let reason: String
        switch state {
        case .opening: reason = "Opening your wallet…"
        case .balance(let sats):
            reason = "Send and receive bitcoin."
            NSLog("OpenAgents wallet: balance shown (%llu sats)", sats)
        case .noWallet: reason = "No wallet on this phone yet."
        case .locked: reason = "Your wallet is locked. Unlock your phone, then reopen OpenAgents."
        case .unreadable: reason = "The wallet saved on this phone can't be read. It has been left as it is."
        case .failed(let message): reason = message
        }
        if case .balance(let sats) = state { balanceLabel.text = "₿" + String(sats) } else { balanceLabel.text = "₿–" }
        walletButton.accessibilityValue = balanceLabel.text
        walletButton.accessibilityHint = reason
    }
    @objc private func motionChanged() { metareal_home_resume(handle) }
    fileprivate func frame(_ link: CADisplayLink) { updateScene(at: link.timestamp) }
    private func updateScene(at timestamp: Double) {
        guard renderer != nil, lastSize.width > 0, lastSize.height > 0 else { return }
        if inGrid { return updateGrid(at: timestamp) }
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
        beginArrival()
    }
    private func loadCast() throws -> CastAssets {
        guard let renderer, let url = Bundle.main.url(forResource: "Cast", withExtension: "bin") else { throw HomeError.resource("Cast.bin") }
        let data = try Data(contentsOf: url)
        let bytes = UnsafeMutableRawBufferPointer.allocate(byteCount: data.count, alignment: 16)
        data.copyBytes(to: bytes)
        castBytes = bytes
        let cast = metareal_cast_open(castView(bytes))
        palette = Array(repeating: coax_t_Joint(), count: Int(metareal_cast_palette(cast)))
        bodies = Array(repeating: 0, count: Int(metareal_cast_robots(cast) * cast.robot_bodies.count) * 7)
        let layers = try [5, 9, 10, 11, 12, 16].map { layer -> URL in
            guard let url = Bundle.main.url(forResource: "cast-layer\(layer)", withExtension: "jpg") else { throw HomeError.resource("cast-layer\(layer).jpg") }
            return url
        }
        return try CastAssets(device: renderer.device, queue: renderer.commandQueue, bytes: data, cast: cast, layers: layers)
    }
    private func castView(_ bytes: UnsafeMutableRawBufferPointer) -> coax_v_u8 {
        coax_v_u8(data: bytes.baseAddress!.assumingMemoryBound(to: UInt8.self), count: Int64(bytes.count))
    }
    /// The Grid, its cast and you, made at launch so the home can show them.
    private func setUpGrid() {
        guard !inGrid else { return }
        if cast == nil {
            do { cast = try loadCast() } catch { NSLog("OpenAgents: the Grid's cast is unavailable: %@", String(describing: error)) }
        }
        anim = coax_t_Anim(move: 0, start: 0, from: 0, from_start: 0, fade_start: -1, air: false, dance: false)
        gridStart = nil
        var config: UInt8 = 0
        gridHandle = mr_create(&config, 1, 1)
        guard gridHandle != 0 else { NSLog("OpenAgents: no storage for the Grid"); return }
        player = metareal_player_place(0, 0, 0, 0)
        walls = Array(repeating: 0, count: 64)
        let wallCount = walls.withUnsafeMutableBufferPointer { Int(metareal_grid_walls(coax_v_f32(data: $0.baseAddress, count: Int64($0.count)))) }
        walls = Array(walls.prefix(wallCount * 6))
        sticks = metareal_sticks_reset(); gridTouches = [:]; lastTick = nil
        let element = UIAccessibilityElement(accessibilityContainer: surface)
        element.accessibilityLabel = "The Grid"
        element.accessibilityHint = "Drag on the left to walk, drag on the right to look, tap on the right to jump."
        let dance = HomeActionElement(accessibilityContainer: surface)
        dance.accessibilityLabel = "Dance"
        dance.accessibilityTraits = .button
        dance.accessibilityIdentifier = "grid.dance"
        dance.activate = { [weak self] in self?.danceRequested = true; return self != nil }
        danceElement = dance
        element.accessibilityIdentifier = "grid.world"
        element.accessibilityFrameInContainerSpace = view.bounds
        gridElement = element
        let you = UIAccessibilityElement(accessibilityContainer: surface)
        you.accessibilityLabel = "You"
        you.accessibilityIdentifier = "grid.you"
        you.accessibilityFrameInContainerSpace = CGRect(x: view.bounds.midX - 30, y: view.bounds.midY - 60, width: 60, height: 120)
        youElement = you
    }
    /// Start: the camera leaves the overview where it is and dollies down.
    private func beginArrival() {
        guard inGrid, case .overview = stage, let element = gridElement else { return }
        let now = CACurrentMediaTime()
        let from = lastShot ?? metareal_camera_overview(0)
        stage = .arriving(metareal_camera_dolly_start(from, now, UIAccessibility.isReduceMotionEnabled))
        trackedTouch = nil
        element.accessibilityValue = "arriving"
        view.accessibilityElements = [element] + [youElement].compactMap { cast == nil ? nil : $0 }
        UIAccessibility.post(notification: .screenChanged, argument: element)
        NSLog("OpenAgents: arriving in the Grid")
    }
    /// The dolly is over: the sticks, the hotbar and your balance appear.
    private func arrive() {
        stage = .playing
        UserDefaults.standard.set(true, forKey: "OAGridSeen")
        sticks = metareal_sticks_reset(); gridTouches = [:]
        view.isMultipleTouchEnabled = true
        balanceLabel.isHidden = false; walletButton.isHidden = false; voice.isHidden = false
        view.accessibilityElements = ([gridElement, youElement, danceElement, walletButton, housePortal].compactMap { $0 }.filter { $0 !== youElement || cast != nil }) + voice.accessibleParts
        UIAccessibility.post(notification: .screenChanged, argument: gridElement)
        NSLog("OpenAgents: entered the Grid")
    }
    private func point(_ v: coax_t_Vec3) -> coax_t_Point { coax_t_Point(x: v.x, y: v.y, z: v.z) }
    private func updateGrid(at timestamp: Double) {
        #if DEBUG
        let capture = R0FrameCapture.shared
        let captureFrame = capture?.beginFrame(active: playing) ?? -1
        let frameClock = capture == nil ? 0 : R0FrameCapture.clock()
        defer { capture?.endFrame(captureFrame, since: frameClock) }
        if captureFrame >= 0 {
            let input = metareal_bench_frame(Int64(captureFrame))
            sticks.pad_y = input.pad_y
            sticks.run = input.run; sticks.jump = input.rising; sticks.dance = input.dance
        }
        #endif
        let frame = Float(min(max(timestamp - (lastTick ?? timestamp), 0), 0.25)); lastTick = timestamp
        if playing {
            player = metareal_player_input(player, 0, 0, sticks.pad_x, sticks.pad_y, sticks.run, sticks.jump, sticks.dyaw, sticks.dpitch)
            if sticks.dance || danceRequested { anim = metareal_cast_dance(anim); danceRequested = false }
            sticks = metareal_sticks_used(sticks)
        }
        // The robots move first; then you, among their bodies.
        #if DEBUG
        let robotsClock = capture == nil ? 0 : R0FrameCapture.clock()
        #endif
        var placed = 0, posed = false
        if let cast, let bytes = castBytes {
            if gridStart == nil { gridStart = timestamp }
            let now = timestamp - (gridStart ?? timestamp), b = castView(bytes), c = cast.cast
            placed = palette.withUnsafeMutableBufferPointer { joints in
                let view = coax_v_t_Joint(data: joints.baseAddress, count: Int64(joints.count))
                posed = metareal_cast_robots_pose(b, c, now, view)
                guard posed else { return 0 }
                return bodies.withUnsafeMutableBufferPointer {
                    Int(metareal_cast_bodies(b, c, view, coax_v_f32(data: $0.baseAddress, count: Int64($0.count))))
                }
            }
        }
        #if DEBUG
        capture?.elapsed("robots_and_bodies", frame: captureFrame, since: robotsClock)
        let playerClock = capture == nil ? 0 : R0FrameCapture.clock()
        #endif
        player = bodies.withUnsafeMutableBufferPointer { b in walls.withUnsafeMutableBufferPointer { w in
            metareal_player_advance_among(player, frame, coax_v_f32(data: b.baseAddress, count: Int64(placed * 7)),
                                          coax_v_f32(data: w.baseAddress, count: Int64(w.count)))
        } }
        #if DEBUG
        capture?.elapsed("player_update", frame: captureFrame, since: playerClock)
        #endif
        let feet = metareal_player_feet(player), face = metareal_player_face(player)
        // The camera: the overview, the dolly down to you, or your own.
        let follow = coax_t_Shot(eye: point(metareal_player_eye(player)), at: point(metareal_player_at(player)))
        var shot = follow
        switch stage {
        case .overview:
            if overviewEpoch == nil { overviewEpoch = timestamp }
            shot = metareal_camera_overview(timestamp - (overviewEpoch ?? timestamp))
        case .arriving(let d):
            shot = metareal_camera_dolly(d, timestamp, follow)
            if metareal_camera_dolly_done(d, timestamp) { arrive() }
        case .playing: break
        }
        lastShot = shot
        let eye = shot.eye, at = shot.at
        let width = Float(lastSize.width), height = Float(lastSize.height), scale = surface.metalLayer.contentsScale
        mr_clear_frame(gridHandle)
        metareal_grid_frame(gridHandle, sticks, eye.x, eye.y, eye.z, at.x, at.y, at.z, feet.x, feet.y, feet.z,
                            width, height, Float(scale), anim.dance, timestamp, playing)
        let portal = metareal_grid_portal(gridHandle, eye.x, eye.y, eye.z, at.x, at.y, at.z, width, height, Float(scale))
        housePortal.isHidden = !playing || !portal.x.isFinite || !portal.y.isFinite || portal.x < 0 || portal.x > width || portal.y < 0 || portal.y > height
        housePortal.frame = CGRect(x: CGFloat(portal.x)/scale - 85, y: CGFloat(portal.y)/scale - 35, width: 170, height: 44)
        // Over the overview, the home's Start bar.
        var bar: Int64?
        if case .overview = stage {
            mr_clear_frame(handle)
            metareal_home_hud_frame(handle, timestamp, lastSize.width, lastSize.height, scale, view.safeAreaInsets.bottom * scale)
            guard mr_ok(handle) else { suspend(); NSLog("OpenAgents: invalid home draw packet"); return }
            bar = handle
            if let b = mr_bar(handle)?.pointee {
                startElement?.accessibilityFrameInContainerSpace = CGRect(x: CGFloat(b.x)/scale, y: CGFloat(b.y)/scale, width: CGFloat(b.width)/scale, height: CGFloat(b.height)/scale)
            }
        }
        if let slots = mr_slots(gridHandle), mr_slot_count(gridHandle) > 0 {
            let s = slots[0], k = surface.metalLayer.contentsScale
            let rect = CGRect(x: CGFloat(s.x) / k, y: CGFloat(s.y) / k, width: CGFloat(s.width) / k, height: CGFloat(s.height) / k)
            if danceElement?.accessibilityFrameInContainerSpace != rect { danceElement?.accessibilityFrameInContainerSpace = rect }
        }
        guard mr_ok(gridHandle) else { suspend(); NSLog("OpenAgents: invalid Grid draw packet"); return }
        var castFrame: CastFrame?
        if let cast, let bytes = castBytes {
            let now = timestamp - (gridStart ?? timestamp), b = castView(bytes), c = cast.cast
            anim = metareal_cast_update(b, c, anim, metareal_player_act(player), player.vel.y > 0, now)
            var matrix = [Float](repeating: 0, count: 16)
            let viewed = matrix.withUnsafeMutableBufferPointer { metareal_grid_view(eye.x, eye.y, eye.z, at.x, at.y, at.z, width, height, coax_v_f32(data: $0.baseAddress, count: 16)) }
            let anim = anim, robots = palette, joints = Int(c.joints)
            if viewed && posed {
                castFrame = CastFrame(view: matrix) { out in
                    guard out.count >= robots.count, metareal_cast_pose(b, c, anim, now, feet.x, feet.y, feet.z, face, out) else { return false }
                    robots.withUnsafeBufferPointer { (out.data! + joints).update(from: $0.baseAddress! + joints, count: robots.count - joints) }
                    return true
                }
            }
            let names = ["idle", "walking", "running", "jump start", "jump loop", "landing", "dancing"]
            let name = names[min(Int(anim.move), names.count - 1)]
            if youElement?.accessibilityValue != name { youElement?.accessibilityValue = name }
        }
        renderer?.draw(handle: gridHandle, time: timestamp, cast: cast, frame: castFrame, bar: bar)
        guard playing else { return }
        if metareal_haunt_portal(feet.x, feet.z) { enterHouse(); return }
        #if DEBUG
        if capture?.finished == true { gridElement?.accessibilityValue = "benchmark.complete"; return }
        #endif
        let place = String(format: "%.1f %.1f", feet.x, feet.z)
        if gridElement?.accessibilityValue != place { gridElement?.accessibilityValue = place }
    }
    private func enterHouse() {
        guard house == nil, playing, let window = view.window else { return }
        voice.stop()
        suspend()
        let screen = HauntViewController(); house = screen
        // Each world owns the window's full Metal surface. Nesting Metal
        // layers inside the suspended Grid clips their rotation snapshots.
        screen.closed = { [self, weak screen, weak window] in
            screen?.closed = nil
            self.house = nil
            self.player = metareal_player_place(-1.5, 0, 7, .pi)
            window?.rootViewController = self
            self.resume()
        }
        window.rootViewController = screen
        UIAccessibility.post(notification: .screenChanged, argument: screen.view)
    }
    private func routeGrid(_ touches: Set<UITouch>, _ phase: Int64) {
        let scale = surface.metalLayer.contentsScale
        for touch in touches {
            let id = ObjectIdentifier(touch)
            if phase == 0 { gridTouches[id] = nextTouchSlot; nextTouchSlot += 1 }
            guard let slot = gridTouches[id] else { continue }
            let point = touch.location(in: view)
            sticks = metareal_sticks_touch(sticks, slot, phase, Float(point.x * scale), Float(point.y * scale),
                                           Float(lastSize.width), Float(lastSize.height), Float(scale), touch.timestamp)
            if phase >= 2 { gridTouches[id] = nil }
        }
    }
    private func activateAccessibleButton() -> Bool {
        guard let bar = mr_bar(handle)?.pointee, bar.width > 0 else { return false }
        let x = Double(bar.x+bar.width/2), y = Double(bar.y+bar.height/2)
        _ = metareal_home_touch(handle, 0, x, y)
        if metareal_home_touch(handle, 2, x, y) { publishInvocation(); return true }
        return false
    }
    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        if playing { return routeGrid(touches, 0) }
        // A touch during the dolly hurries it.
        if case .arriving(let d) = stage {
            let follow = coax_t_Shot(eye: point(metareal_player_eye(player)), at: point(metareal_player_at(player)))
            stage = .arriving(metareal_camera_dolly_hurry(d, CACurrentMediaTime(), follow))
            return
        }
        guard trackedTouch == nil, let touch = touches.first else { return }
        trackedTouch = touch; route(0, touch)
    }
    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        if playing { return routeGrid(touches, 1) }
        if let touch = trackedTouch, touches.contains(touch) { route(1, touch) }
    }
    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        if playing { return routeGrid(touches, 2) }
        if let touch = trackedTouch, touches.contains(touch) { route(2, touch); trackedTouch = nil }
    }
    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        if playing { return routeGrid(touches, 3) }
        if let touch = trackedTouch { route(3, touch) }; trackedTouch = nil
    }
    deinit {
        clock?.invalidate(); NotificationCenter.default.removeObserver(self); mr_destroy(handle)
        if gridHandle != 0 { mr_destroy(gridHandle) }
        castBytes?.deallocate()
    }
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
