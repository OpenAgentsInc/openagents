import AVFoundation
import UIKit

/// The Grid's voice controls, drawn like the hotbar's slots and the balance:
/// one toggle (tap to talk hands-free, tap again to stop), a small level and
/// turn indicator driven by every frame's FrameFeatures, and a caption line
/// that shows the partial, then final transcript, then the reply as it's spoken.
/// A long press on the toggle sets the voice server.
final class VoiceBar: UIView {
    let toggle = UIButton(type: .custom)
    private let level = VoiceLevelView()
    private let caption = UILabel()
    private let captionBox = UIView()
    private let debug = UILabel()
    private var session: VoiceSession?
    private var clock: CADisplayLink?
    weak var presenter: UIViewController?

    static var defaultHost: String? {
        #if targetEnvironment(simulator)
        return "ws://127.0.0.1:8787" // the Mac's voice host: metareal/voice-host/run.sh
        #else
        return nil
        #endif
    }
    /// `-OAVoiceHost ws://…` on launch, or the long press; else the simulator default.
    static var host: URL? {
        let s = UserDefaults.standard.string(forKey: "OAVoiceHost") ?? defaultHost
        return s.flatMap(URL.init(string:)).flatMap { ["ws", "wss"].contains($0.scheme ?? "") ? $0 : nil }
    }
    #if DEBUG
    /// Test hooks, debug builds only: `-OAVoiceFixture NAME` streams a bundled
    /// recording instead of the microphone; `-OAVoiceObserve YES` shows frame
    /// measurements and asks the host for its own.
    static var fixture: String? { UserDefaults.standard.string(forKey: "OAVoiceFixture") }
    static var observe: Bool { UserDefaults.standard.bool(forKey: "OAVoiceObserve") }
    #else
    static var fixture: String? { nil }
    static var observe: Bool { false }
    #endif
    /// One identity per install, sent as the Hello's speaker.
    static var speaker: String {
        if let id = UserDefaults.standard.string(forKey: "OAVoiceSpeaker") { return id }
        let id = "ios-" + UUID().uuidString.prefix(8).lowercased()
        UserDefaults.standard.set(id, forKey: "OAVoiceSpeaker")
        return id
    }

    override init(frame: CGRect) {
        super.init(frame: frame)
        toggle.translatesAutoresizingMaskIntoConstraints = false
        toggle.setImage(UIImage(systemName: "mic"), for: .normal)
        toggle.tintColor = .white
        toggle.backgroundColor = UIColor(white: 0.02, alpha: 0.65); toggle.layer.cornerRadius = 10
        toggle.layer.borderWidth = 1; toggle.layer.borderColor = UIColor(white: 0.8, alpha: 0.4).cgColor
        toggle.accessibilityIdentifier = "voice.toggle"
        toggle.accessibilityLabel = "Voice"
        toggle.accessibilityValue = "off"
        toggle.accessibilityHint = "Talk hands-free. Tap again to stop."
        toggle.addAction(UIAction { [weak self] _ in self?.toggled() }, for: .touchUpInside)
        let hold = UILongPressGestureRecognizer(target: self, action: #selector(held(_:)))
        hold.minimumPressDuration = 0.8
        toggle.addGestureRecognizer(hold)
        addSubview(toggle)

        level.translatesAutoresizingMaskIntoConstraints = false
        level.isAccessibilityElement = true
        level.accessibilityIdentifier = "voice.level"
        level.accessibilityLabel = "Voice level"
        level.accessibilityValue = "level 0 peak 0"
        level.isHidden = true
        addSubview(level)

        captionBox.translatesAutoresizingMaskIntoConstraints = false
        captionBox.backgroundColor = UIColor(white: 0.02, alpha: 0.65); captionBox.layer.cornerRadius = 10
        captionBox.layer.borderWidth = 1; captionBox.layer.borderColor = UIColor(white: 0.8, alpha: 0.25).cgColor
        captionBox.isHidden = true
        addSubview(captionBox)
        caption.translatesAutoresizingMaskIntoConstraints = false
        caption.font = .systemFont(ofSize: 12) // Michroma once it is registered: didMoveToWindow
        caption.textColor = UIColor(white: 1, alpha: 0.9)
        caption.numberOfLines = 2
        caption.lineBreakMode = .byTruncatingHead
        caption.accessibilityIdentifier = "voice.caption"
        captionBox.addSubview(caption)

        debug.translatesAutoresizingMaskIntoConstraints = false
        debug.font = .monospacedSystemFont(ofSize: 9, weight: .regular)
        debug.textColor = UIColor(white: 1, alpha: 0.7)
        debug.numberOfLines = 3
        debug.accessibilityIdentifier = "voice.observe"
        debug.isHidden = true
        addSubview(debug)

        NSLayoutConstraint.activate([
            toggle.topAnchor.constraint(equalTo: topAnchor),
            toggle.leadingAnchor.constraint(equalTo: leadingAnchor),
            toggle.widthAnchor.constraint(equalToConstant: 44),
            toggle.heightAnchor.constraint(equalToConstant: 44),
            level.leadingAnchor.constraint(equalTo: toggle.trailingAnchor, constant: 8),
            level.centerYAnchor.constraint(equalTo: toggle.centerYAnchor),
            level.widthAnchor.constraint(equalToConstant: 30),
            level.heightAnchor.constraint(equalToConstant: 22),
            captionBox.topAnchor.constraint(equalTo: toggle.bottomAnchor, constant: 8),
            captionBox.leadingAnchor.constraint(equalTo: leadingAnchor),
            captionBox.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor),
            caption.topAnchor.constraint(equalTo: captionBox.topAnchor, constant: 8),
            caption.bottomAnchor.constraint(equalTo: captionBox.bottomAnchor, constant: -8),
            caption.leadingAnchor.constraint(equalTo: captionBox.leadingAnchor, constant: 10),
            caption.trailingAnchor.constraint(equalTo: captionBox.trailingAnchor, constant: -10),
            debug.topAnchor.constraint(equalTo: captionBox.bottomAnchor, constant: 6),
            debug.leadingAnchor.constraint(equalTo: leadingAnchor),
            debug.trailingAnchor.constraint(equalTo: trailingAnchor),
            debug.bottomAnchor.constraint(lessThanOrEqualTo: bottomAnchor),
        ])
    }
    required init?(coder: NSCoder) { fatalError() }

    /// HomeRenderer registers Michroma from the bundle after this view is
    /// made (it is a stored property of the home), so the font is looked up
    /// once the bar is on screen, like the balance after the renderer.
    override func didMoveToWindow() {
        super.didMoveToWindow()
        if window != nil, let michroma = UIFont(name: "Michroma", size: 11) { caption.font = michroma }
    }

    /// Only the controls take touches; the Grid gets the rest.
    override func point(inside point: CGPoint, with event: UIEvent?) -> Bool {
        toggle.frame.contains(point) || (!captionBox.isHidden && captionBox.frame.contains(point))
    }

    var accessibleParts: [Any] { [toggle, level, caption] }
    var isOn: Bool { session != nil }

    // MARK: on / off

    private func toggled() {
        if session != nil { stop(); return }
        // The first time, say plainly where speech goes.
        if !UserDefaults.standard.bool(forKey: "OAVoiceNoticeSeen") {
            let alert = UIAlertController(title: "Voice",
                message: "When voice is on, what you say goes to our voice server, which uses Google to turn it into text and to speak replies. We don't keep the audio.",
                preferredStyle: .alert)
            alert.addAction(UIAlertAction(title: "Not now", style: .cancel))
            alert.addAction(UIAlertAction(title: "Turn on", style: .default) { [weak self] _ in
                UserDefaults.standard.set(true, forKey: "OAVoiceNoticeSeen"); self?.toggled()
            })
            presenter?.present(alert, animated: true)
            return
        }
        if Self.fixture != nil { return begin() } // a recording, not the microphone
        switch AVAudioApplication.shared.recordPermission {
        case .granted: begin()
        case .denied: deniedMicrophone()
        default:
            AVAudioApplication.requestRecordPermission { [weak self] granted in
                DispatchQueue.main.async { granted ? self?.begin() : self?.deniedMicrophone() }
            }
        }
    }

    private func deniedMicrophone() {
        let alert = UIAlertController(title: "Microphone is off",
            message: "Voice needs the microphone. Turn it on for OpenAgents in Settings.", preferredStyle: .alert)
        alert.addAction(UIAlertAction(title: "Not now", style: .cancel))
        alert.addAction(UIAlertAction(title: "Open Settings", style: .default) { _ in
            if let url = URL(string: UIApplication.openSettingsURLString) { UIApplication.shared.open(url) }
        })
        presenter?.present(alert, animated: true)
    }

    private func begin() {
        let s = VoiceSession(options: .init(host: Self.host, fixture: Self.fixture, observe: Self.observe, speaker: Self.speaker))
        s.onChange = { [weak self] in self?.refresh() }
        session = s
        s.start()
        toggle.setImage(UIImage(systemName: "mic.fill"), for: .normal)
        toggle.backgroundColor = UIColor(white: 0.9, alpha: 0.85); toggle.tintColor = .black
        level.isHidden = false
        debug.isHidden = !Self.observe
        let link = CADisplayLink(target: VoiceTick(owner: self), selector: #selector(VoiceTick.tick))
        link.preferredFrameRateRange = CAFrameRateRange(minimum: 15, maximum: 30, preferred: 30)
        link.add(to: .main, forMode: .common); clock = link
        refresh()
        NSLog("OpenAgents voice: on (%@%@)", Self.host?.absoluteString ?? "no server", Self.fixture.map { ", fixture \($0)" } ?? "")
    }

    func stop() {
        guard let s = session else { return }
        s.onChange = nil; s.stop(); session = nil
        clock?.invalidate(); clock = nil
        toggle.setImage(UIImage(systemName: "mic"), for: .normal)
        toggle.backgroundColor = UIColor(white: 0.02, alpha: 0.65); toggle.tintColor = .white
        toggle.accessibilityValue = "off"
        level.isHidden = true; captionBox.isHidden = true; debug.isHidden = true
        caption.text = nil; caption.accessibilityLabel = nil; caption.accessibilityValue = nil
        NSLog("OpenAgents voice: off")
    }

    @objc private func held(_ g: UILongPressGestureRecognizer) {
        guard g.state == .began else { return }
        let alert = UIAlertController(title: "Voice server", message: "A ws:// or wss:// address.", preferredStyle: .alert)
        alert.addTextField { f in
            f.text = Self.host?.absoluteString; f.placeholder = "ws://127.0.0.1:8787"
            f.keyboardType = .URL; f.autocapitalizationType = .none; f.autocorrectionType = .no
            f.accessibilityIdentifier = "voice.host"
        }
        alert.addAction(UIAlertAction(title: "Cancel", style: .cancel))
        alert.addAction(UIAlertAction(title: "Save", style: .default) { [weak self, weak alert] _ in
            let text = alert?.textFields?.first?.text?.trimmingCharacters(in: .whitespaces) ?? ""
            if text.isEmpty { UserDefaults.standard.removeObject(forKey: "OAVoiceHost") }
            else { UserDefaults.standard.set(text, forKey: "OAVoiceHost") }
            // Reconnect to the new server if voice is on.
            if self?.session != nil { self?.stop(); self?.begin() }
        })
        presenter?.present(alert, animated: true)
    }

    // MARK: drawing

    fileprivate func refresh() {
        guard let s = session?.snapshot else { return }
        let live = VoiceSession.level(s.uplink.rms_db)
        let downlinkPlaying = s.downlink.playback_state == UInt32(OA_PLAYBACK_STATE_PLAYING.rawValue)
        level.update(level: live, turn: s.uplink.turn_state, replying: s.phase == .replying || downlinkPlaying)
        let value = "level \(Int(live * 100)) peak \(Int(s.peakLevel * 100)) turns \(s.turns) replies \(s.replies)"
        if level.accessibilityValue != value { level.accessibilityValue = value }
        if toggle.accessibilityValue != s.phase.rawValue { toggle.accessibilityValue = s.phase.rawValue }
        let text = s.caption
        if caption.text != text {
            caption.text = text
            caption.accessibilityLabel = text
        }
        caption.textColor = UIColor(white: 1, alpha: s.captionKind == .partial ? 0.6 : 0.92)
        let value2 = "\(s.captionKind.rawValue); heard: \(s.heard)"
        if caption.accessibilityValue != value2 { caption.accessibilityValue = value2 }
        captionBox.isHidden = text.isEmpty
        if !debug.isHidden {
            let u = s.uplink
            var lines = [String(format: "up #%u ep %llu rms %.0f pk %.0f sp %.2f nf %.0f zc %.2f turn %u %ums/%ums",
                                u.sequence, u.media_epoch, u.rms_db, u.peak_db, u.speech, u.noise_db, u.zero_crossings,
                                u.turn_state, u.speech_ms, u.silence_ms)]
            lines.append("\(s.provider) · frames \(s.frames) · \(s.lastDecision)")
            if let m = s.meter { lines.append("meter \(m.recognized_ms) ms, \(m.synthesized_chars) chars, free \(m.free_ms_left) ms left") }
            debug.text = lines.joined(separator: "\n")
        }
    }
}

/// Five bars for the level, lit by the turn: dim while idle, bright while
/// you speak, half while the turn trails off, and outlined while a reply plays.
final class VoiceLevelView: UIView {
    private var bars: [CALayer] = []
    private var shown: Float = 0
    override init(frame: CGRect) {
        super.init(frame: frame)
        for _ in 0..<5 { let b = CALayer(); b.cornerRadius = 1.5; layer.addSublayer(b); bars.append(b) }
    }
    required init?(coder: NSCoder) { fatalError() }
    func update(level: Float, turn: UInt32, replying: Bool) {
        // Fast up, slow down, so a word is visible.
        shown = level > shown ? level : max(level, shown - 0.04)
        let speaking = turn == UInt32(OA_TURN_STATE_SPEAKING.rawValue)
        let trailing = turn == UInt32(OA_TURN_STATE_TRAILING.rawValue)
        let alpha: CGFloat = speaking ? 1 : (trailing ? 0.7 : 0.45)
        CATransaction.begin(); CATransaction.setDisableActions(true)
        let w = bounds.width / 5
        for (i, b) in bars.enumerated() {
            let lit = shown * 5 > Float(i)
            let h = bounds.height * CGFloat(0.3 + 0.7 * Double(i + 1) / 5)
            b.frame = CGRect(x: CGFloat(i) * w + 1, y: bounds.height - h, width: w - 2, height: h)
            b.backgroundColor = UIColor(white: 1, alpha: lit ? alpha : 0.12).cgColor
            b.borderWidth = replying ? 1 : 0
            b.borderColor = UIColor(white: 1, alpha: 0.6).cgColor
        }
        CATransaction.commit()
    }
}

private final class VoiceTick: NSObject {
    weak var owner: VoiceBar?
    init(owner: VoiceBar) { self.owner = owner }
    @objc func tick() { owner?.refresh() }
}
