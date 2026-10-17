import AVFoundation
import AVKit
import SafariServices
import UIKit

/// Lab card rows (#11884). Rust lays the card out from `lab-ui`'s Rust Native
/// card; these views only fill its media rect with the cell for the card's
/// standard view and run its open and note buttons. Files come from the
/// saved history's `labMedia`, which reads only the card's own files through
/// the signed-in account.
@MainActor
final class LabCells {
    private(set) weak var history: CoreSavedHistory?
    private var cells: [String: LabMediaCell] = [:]
    private var observers: [NSObjectProtocol] = []

    init() {
        let center = NotificationCenter.default
        observers.append(center.addObserver(forName: UIApplication.didEnterBackgroundNotification, object: nil, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.pauseAll() }
        })
    }

    deinit {
        observers.forEach(NotificationCenter.default.removeObserver)
    }

    /// Binds the cells to the account's saved history. A different history
    /// (an account switch) or none (sign-out) drops every cell.
    func bind(_ history: CoreSavedHistory?) {
        if history !== self.history { clear() }
        self.history = history
    }

    /// The live cell for a card, kept across row reuse so playback survives.
    func cell(card: String, view: LabMediaView) -> UIView {
        if let cell = cells[card], cell.view == view { return cell }
        let cell = LabMediaCell(card: card, view: view, cells: self)
        cells[card] = cell
        return cell
    }

    /// The playing time of a card's video, if it has one loaded.
    func seconds(card: String) -> Double? {
        cells[card]?.seconds
    }

    func pauseAll() {
        cells.values.forEach { $0.pause() }
    }

    /// Stops every player and forgets every picture.
    func clear() {
        cells.values.forEach { $0.release() }
        cells.removeAll()
    }

    func open(card: String, from view: UIView) {
        guard let page = history?.labPage(card: card), let url = URL(string: page),
              url.scheme == "https" || url.scheme == "http", let vc = view.findViewController() else { return }
        pauseAll()
        let safari = SFSafariViewController(url: url)
        vc.present(safari, animated: true)
    }

    /// Asks for a note (the keyboard's microphone dictates it) and adds it at
    /// the video's playing time when there is one.
    func note(card: String, from view: UIView) {
        guard let vc = view.findViewController() else { return }
        let seconds = seconds(card: card)
        let alert = UIAlertController(
            title: "Add a note",
            message: seconds.map { "At \(LabCells.clock($0))" },
            preferredStyle: .alert
        )
        alert.addTextField { field in
            field.placeholder = "Your note"
            field.autocapitalizationType = .sentences
            field.accessibilityIdentifier = "lab-note-text"
        }
        alert.addAction(UIAlertAction(title: "Cancel", style: .cancel))
        alert.addAction(UIAlertAction(title: "Add", style: .default) { [weak self, weak alert] _ in
            let text = alert?.textFields?.first?.text ?? ""
            Task { @MainActor [weak self] in
                guard let history = self?.history else { return }
                do { try await history.labAddNote(card: card, seconds: seconds, text: text) }
                catch { Toast.show(LabCells.message(error), in: view.window) }
            }
        })
        vc.present(alert, animated: true)
    }

    fileprivate func media(card: String, artifact: String) async -> LabMediaFile {
        guard let history else { return .missing }
        return await history.labMedia(card: card, artifact: artifact)
    }

    fileprivate func visible(card: String, _ visible: Bool) {
        history?.labVisible(card: card, visible: visible)
    }

    static func clock(_ seconds: Double) -> String {
        let total = max(0, Int(seconds.isFinite ? seconds : 0))
        let (h, m, s) = (total / 3600, total / 60 % 60, total % 60)
        return h > 0 ? String(format: "%d:%02d:%02d", h, m, s) : String(format: "%d:%02d", m, s)
    }

    private static func message(_ error: Error) -> String {
        switch error as? CoreError {
        case let .InvalidArgument(message)?, let .Auth(message)?, let .Network(message)?: message
        default: "Couldn’t add the note. Try again."
        }
    }
}

/// A Lab card's picture: one cell per standard view.
final class LabMediaCell: UIView {
    let card: String
    let view: LabMediaView
    private weak var cells: LabCells?
    private var files: LabCardFiles?
    private let main = UIImageView()
    private let sideA = UIImageView()
    private let sideB = UIImageView()
    private let labelA = UILabel()
    private let labelB = UILabel()
    private let strip = UIScrollView()
    private var stripViews: [UIImageView] = []
    private let play = UIButton(type: .system)
    private let full = UIButton(type: .system)
    private let openOverlay = UIButton(type: .system)
    private var player: AVPlayer?
    private var playerLayer: AVPlayerLayer?
    private var videoURL: URL?
    private var loads: [Task<Void, Never>] = []
    private var released = false

    init(card: String, view: LabMediaView, cells: LabCells) {
        self.card = card
        self.view = view
        self.cells = cells
        super.init(frame: .zero)
        clipsToBounds = true
        layer.cornerRadius = 10
        layer.cornerCurve = .continuous
        accessibilityIdentifier = "lab-media-\(LabMediaCell.name(view))"
        for iv in [main, sideA, sideB] {
            iv.contentMode = .scaleAspectFit
            iv.clipsToBounds = true
            iv.backgroundColor = UIColor.black.withAlphaComponent(0.35)
            iv.layer.cornerRadius = 8
            iv.layer.cornerCurve = .continuous
            iv.accessibilityTraits = .image
        }
        for l in [labelA, labelB] {
            l.font = .preferredFont(forTextStyle: .caption1)
            l.textColor = Palette.secondary
            l.textAlignment = .center
        }
        strip.showsHorizontalScrollIndicator = false
        switch view {
        case .compare:
            [sideA, sideB, labelA, labelB].forEach(addSubview)
        default:
            addSubview(main)
            addSubview(strip)
        }
        if view == .video {
            play.configuration = .filled()
            play.configuration?.image = UIImage(systemName: "play.fill")
            play.configuration?.cornerStyle = .capsule
            play.accessibilityLabel = "Play"
            play.accessibilityIdentifier = "lab-play"
            play.addAction(UIAction { [weak self] _ in self?.toggle() }, for: .touchUpInside)
            full.configuration = .tinted()
            full.configuration?.image = UIImage(systemName: "arrow.up.left.and.arrow.down.right")
            full.configuration?.cornerStyle = .capsule
            full.accessibilityLabel = "Full screen"
            full.accessibilityIdentifier = "lab-full-screen"
            full.addAction(UIAction { [weak self] _ in self?.fullScreen() }, for: .touchUpInside)
            play.isHidden = true
            full.isHidden = true
            addSubview(play)
            addSubview(full)
        }
        if view == .model3d || view == .playable {
            // The phone shows the poster; the model or game opens on its page.
            openOverlay.configuration = .tinted()
            openOverlay.configuration?.title = view == .model3d ? "View model" : "Play"
            openOverlay.configuration?.cornerStyle = .capsule
            openOverlay.accessibilityIdentifier = "lab-open-media"
            openOverlay.addAction(UIAction { [weak self] _ in
                guard let self else { return }
                self.cells?.open(card: self.card, from: self)
            }, for: .touchUpInside)
            addSubview(openOverlay)
        }
        load()
    }

    required init?(coder: NSCoder) { fatalError() }

    private static func name(_ view: LabMediaView) -> String {
        switch view {
        case .video: "video"
        case .image: "image"
        case .compare: "compare"
        case .model3d: "model3d"
        case .playable: "playable"
        }
    }

    /// The video's playing time, once it has played.
    var seconds: Double? {
        guard let player else { return nil }
        let t = player.currentTime().seconds
        return t.isFinite ? t : nil
    }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        if window != nil {
            cells?.visible(card: card, true)
            return
        }
        // A row reconfigure moves the cell out and back at once; only a cell
        // still off screen on the next turn stops.
        DispatchQueue.main.async { [weak self] in
            guard let self, self.window == nil else { return }
            self.pause()
            self.cells?.visible(card: self.card, false)
        }
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        let b = bounds
        let gap: CGFloat = 8
        if view == .compare {
            let side = (b.width - gap) / 2
            let label: CGFloat = 19
            let h = max(0, b.height - label)
            sideA.frame = CGRect(x: 0, y: 0, width: side, height: h)
            sideB.frame = CGRect(x: side + gap, y: 0, width: side, height: h)
            labelA.frame = CGRect(x: 0, y: h, width: side, height: label)
            labelB.frame = CGRect(x: side + gap, y: h, width: side, height: label)
            return
        }
        let stripH: CGFloat = stripViews.isEmpty ? 0 : 44
        let screen = CGRect(x: 0, y: 0, width: b.width, height: b.height - (stripH > 0 ? stripH + gap : 0))
        main.frame = screen
        playerLayer?.frame = screen
        strip.frame = CGRect(x: 0, y: screen.maxY + gap, width: b.width, height: stripH)
        var x: CGFloat = 0
        for iv in stripViews {
            iv.frame = CGRect(x: x, y: 0, width: stripH * 16 / 9, height: stripH)
            x += iv.frame.width + 6
        }
        strip.contentSize = CGSize(width: max(0, x - 6), height: stripH)
        play.frame = CGRect(x: screen.midX - 26, y: screen.midY - 26, width: 52, height: 52)
        full.frame = CGRect(x: screen.maxX - 44, y: screen.maxY - 44, width: 36, height: 36)
        openOverlay.sizeToFit()
        openOverlay.frame.origin = CGPoint(x: screen.midX - openOverlay.frame.width / 2, y: screen.maxY - openOverlay.frame.height - 12)
    }

    private func load() {
        guard let files = cells?.history?.labCard(card: card) else { return }
        self.files = files
        labelA.text = files.sideLabels.first
        labelB.text = files.sideLabels.dropFirst().first
        accessibilityLabel = files.title
        stripViews = files.strip.map { _ in
            let iv = UIImageView()
            iv.contentMode = .scaleAspectFill
            iv.clipsToBounds = true
            iv.layer.cornerRadius = 4
            iv.backgroundColor = UIColor.black.withAlphaComponent(0.35)
            strip.addSubview(iv)
            return iv
        }
        if let id = files.main { picture(id, into: main) }
        if let id = files.sideA { picture(id, into: sideA) }
        if let id = files.sideB { picture(id, into: sideB) }
        for (id, iv) in zip(files.strip, stripViews) { picture(id, into: iv) }
        if view == .video, let id = files.plays {
            loads.append(Task { @MainActor [weak self] in
                guard let self, case let .video(url) = await self.cells?.media(card: self.card, artifact: id),
                      !self.released, let link = URL(string: url) else { return }
                self.videoURL = link
                self.play.isHidden = false
                self.full.isHidden = false
            })
        }
        setNeedsLayout()
    }

    private func picture(_ id: String, into iv: UIImageView) {
        loads.append(Task { @MainActor [weak self, weak iv] in
            guard let self, case let .image(bytes) = await self.cells?.media(card: self.card, artifact: id),
                  !self.released else { return }
            // Decode off the main thread; a card's pictures stay small.
            let image = await Task.detached { UIImage(data: bytes)?.preparingForDisplay() }.value
            guard !self.released else { return }
            iv?.image = image
        })
    }

    private func toggle() {
        if let player {
            if player.timeControlStatus == .playing { pause() } else { player.play(); playing(true) }
            return
        }
        guard let videoURL else { return }
        let player = AVPlayer(url: videoURL)
        let layer = AVPlayerLayer(player: player)
        layer.videoGravity = .resizeAspect
        layer.frame = main.frame
        self.layer.insertSublayer(layer, above: main.layer)
        self.player = player
        playerLayer = layer
        try? AVAudioSession.sharedInstance().setCategory(.playback, mode: .moviePlayback)
        player.play()
        playing(true)
    }

    private func playing(_ on: Bool) {
        play.configuration?.image = UIImage(systemName: on ? "pause.fill" : "play.fill")
        play.accessibilityLabel = on ? "Pause" : "Play"
    }

    private func fullScreen() {
        guard let videoURL, let vc = findViewController() else { return }
        let controller = AVPlayerViewController()
        let player = self.player ?? AVPlayer(url: videoURL)
        if self.player == nil { self.player = player }
        controller.player = player
        vc.present(controller, animated: true) { player.play() }
    }

    func pause() {
        player?.pause()
        playing(false)
    }

    /// Stops playing and drops every picture.
    func release() {
        released = true
        loads.forEach { $0.cancel() }
        loads.removeAll()
        player?.pause()
        player?.replaceCurrentItem(with: nil)
        player = nil
        playerLayer?.removeFromSuperlayer()
        playerLayer = nil
        [main, sideA, sideB].forEach { $0.image = nil }
        stripViews.forEach { $0.image = nil }
        cells?.visible(card: card, false)
    }
}
