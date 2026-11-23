// Firm reason: 6 (macOS AppKit Studio Dashboard shell over Coax music/studio.coax & AVAudioEngine).
import AppKit
import Foundation
import UniformTypeIdentifiers

@main
final class MusicStudioAppMain: NSObject, NSApplicationDelegate {
    private var controller: StudioWindowController?

    static func main() {
        let app = NSApplication.shared
        app.setActivationPolicy(.regular)
        let delegate = MusicStudioAppMain()
        app.delegate = delegate
        app.run()
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.appearance = NSAppearance(named: .darkAqua)
        setupMenu()
        controller = StudioWindowController()
        controller?.showWindow(nil)
        NSApp.activate(ignoringOtherApps: true)
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }

    private func setupMenu() {
        let mainMenu = NSMenu()
        let appItem = NSMenuItem()
        let appMenu = NSMenu()
        appMenu.addItem(withTitle: "About OpenAgents Music Studio", action: #selector(NSApplication.orderFrontStandardAboutPanel(_:)), keyEquivalent: "")
        appMenu.addItem(NSMenuItem.separator())
        appMenu.addItem(withTitle: "Quit OpenAgents Music Studio", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        appItem.submenu = appMenu
        mainMenu.addItem(appItem)

        let fileItem = NSMenuItem()
        let fileMenu = NSMenu(title: "File")
        fileMenu.addItem(withTitle: "Import MIDI (.mid)…", action: #selector(StudioWindowController.menuImportMidi), keyEquivalent: "o")
        fileMenu.addItem(withTitle: "Merge MIDI into Movement…", action: #selector(StudioWindowController.menuMergeMidi), keyEquivalent: "m")
        fileMenu.addItem(withTitle: "Export MIDI (.mid)…", action: #selector(StudioWindowController.menuExportMidi), keyEquivalent: "s")
        fileMenu.addItem(withTitle: "Bounce Stereo WAV…", action: #selector(StudioWindowController.menuBounceWav), keyEquivalent: "e")
        fileItem.submenu = fileMenu
        mainMenu.addItem(fileItem)

        let editItem = NSMenuItem()
        let editMenu = NSMenu(title: "Edit")
        editMenu.addItem(withTitle: "Undo", action: #selector(StudioWindowController.menuUndo), keyEquivalent: "z")
        editMenu.addItem(withTitle: "Redo", action: #selector(StudioWindowController.menuRedo), keyEquivalent: "Z")
        editItem.submenu = editMenu
        mainMenu.addItem(editItem)

        NSApp.mainMenu = mainMenu
    }
}

final class StudioWindowController: NSWindowController, NSTextFieldDelegate {
    let store = StudioStore()
    lazy var audio = StudioAudioEngine(store: store)
    lazy var pianoRoll = PianoRollView(store: store, audio: audio)
    lazy var fallingNotes = FallingNotesView(store: store, audio: audio)
    lazy var keyboard = SoftwareKeyboardView(store: store, audio: audio)

    // UI Controls
    private let playBtn = NSButton()
    private let loopBtn = NSButton()
    private let viewSeg = NSSegmentedControl()
    private let toolBtn = NSButton()
    private let clockLabel = NSTextField(labelWithString: "001:01  00:00.0")
    private let bpmKeyLabel = NSTextField(labelWithString: "64 BPM · Db Maj")
    private let bankBtn = NSButton()
    private let statusLabel = NSTextField(labelWithString: "Ready — Press Space to play the 4-Movement Symphonic Starter, click/drag notes, or import .mid")
    private let mvt4ToggleBtn = NSButton()
    private let hotReloadBtn = NSButton()
    private let reverbSlider = NSSlider()

    private let trackStack = NSStackView()
    private let centerContainer = NSView()
    private let remixInput = NSTextField()
    private let remixLog = NSTextView()
    private var movementButtons: [NSButton] = []

    private var uiTimer: Timer?
    private var hotReloadCounter = 0
    private var keyMonitorDown: Any?
    private var keyMonitorUp: Any?
    private var activeQwertyKeys: [UInt16: UInt8] = [:]

    // Mac virtual keyCode -> semitone offset from C
    private let qwertyMap: [UInt16: Int] = [
        0: 0,   // A -> C
        13: 1,  // W -> C#
        1: 2,   // S -> D
        14: 3,  // E -> Eb
        2: 4,   // D -> E
        3: 5,   // F -> F
        17: 6,  // T -> F#
        5: 7,   // G -> G
        16: 8,  // Y -> Ab
        4: 9,   // H -> A
        32: 10, // U -> Bb
        38: 11, // J -> B
        40: 12, // K -> C+1
    ]

    init() {
        let win = NSWindow(
            contentRect: NSRect(x: 80, y: 80, width: 1500, height: 920),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        win.title = "OpenAgents Music Studio (Coax Phase 1 MVP)"
        win.minSize = NSSize(width: 1120, height: 680)
        win.backgroundColor = StudioPalette.bgCanvas
        super.init(window: win)

        buildUI()
        wireCallbacks()
        rebuildTrackList()
        startTimers()
        installKeyboardShortcuts()
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) not supported") }

    deinit {
        uiTimer?.invalidate()
        if let m = keyMonitorDown { NSEvent.removeMonitor(m) }
        if let m = keyMonitorUp { NSEvent.removeMonitor(m) }
        audio.stop()
    }

    // MARK: - Build Studio Layout

    private func buildUI() {
        guard let root = window?.contentView else { return }
        root.wantsLayer = true
        root.layer?.backgroundColor = StudioPalette.bgCanvas.cgColor

        let topBar = buildTopTransportBar()
        let mvtBar = buildMovementAndToolsBar()
        let leftPane = buildLeftTrackRack()
        let rightPane = buildRightRemixDrawer()
        let statusBar = buildStatusBar()

        // Center container holds PianoRollView OR FallingNotesView
        centerContainer.wantsLayer = true
        centerContainer.translatesAutoresizingMaskIntoConstraints = false
        pianoRoll.translatesAutoresizingMaskIntoConstraints = false
        fallingNotes.translatesAutoresizingMaskIntoConstraints = false
        fallingNotes.isHidden = true
        centerContainer.addSubview(pianoRoll)
        centerContainer.addSubview(fallingNotes)
        NSLayoutConstraint.activate([
            pianoRoll.leadingAnchor.constraint(equalTo: centerContainer.leadingAnchor),
            pianoRoll.trailingAnchor.constraint(equalTo: centerContainer.trailingAnchor),
            pianoRoll.topAnchor.constraint(equalTo: centerContainer.topAnchor),
            pianoRoll.bottomAnchor.constraint(equalTo: centerContainer.bottomAnchor),
            fallingNotes.leadingAnchor.constraint(equalTo: centerContainer.leadingAnchor),
            fallingNotes.trailingAnchor.constraint(equalTo: centerContainer.trailingAnchor),
            fallingNotes.topAnchor.constraint(equalTo: centerContainer.topAnchor),
            fallingNotes.bottomAnchor.constraint(equalTo: centerContainer.bottomAnchor),
        ])

        keyboard.translatesAutoresizingMaskIntoConstraints = false

        for v in [topBar, mvtBar, leftPane, centerContainer, rightPane, keyboard, statusBar] {
            v.translatesAutoresizingMaskIntoConstraints = false
            root.addSubview(v)
        }

        NSLayoutConstraint.activate([
            topBar.topAnchor.constraint(equalTo: root.topAnchor),
            topBar.leadingAnchor.constraint(equalTo: root.leadingAnchor),
            topBar.trailingAnchor.constraint(equalTo: root.trailingAnchor),
            topBar.heightAnchor.constraint(equalToConstant: 46),

            mvtBar.topAnchor.constraint(equalTo: topBar.bottomAnchor),
            mvtBar.leadingAnchor.constraint(equalTo: root.leadingAnchor),
            mvtBar.trailingAnchor.constraint(equalTo: root.trailingAnchor),
            mvtBar.heightAnchor.constraint(equalToConstant: 36),

            statusBar.bottomAnchor.constraint(equalTo: root.bottomAnchor),
            statusBar.leadingAnchor.constraint(equalTo: root.leadingAnchor),
            statusBar.trailingAnchor.constraint(equalTo: root.trailingAnchor),
            statusBar.heightAnchor.constraint(equalToConstant: 24),

            keyboard.bottomAnchor.constraint(equalTo: statusBar.topAnchor),
            keyboard.leadingAnchor.constraint(equalTo: root.leadingAnchor),
            keyboard.trailingAnchor.constraint(equalTo: root.trailingAnchor),
            keyboard.heightAnchor.constraint(equalToConstant: 96),

            leftPane.topAnchor.constraint(equalTo: mvtBar.bottomAnchor),
            leftPane.bottomAnchor.constraint(equalTo: keyboard.topAnchor),
            leftPane.leadingAnchor.constraint(equalTo: root.leadingAnchor),
            leftPane.widthAnchor.constraint(equalToConstant: 290),

            rightPane.topAnchor.constraint(equalTo: mvtBar.bottomAnchor),
            rightPane.bottomAnchor.constraint(equalTo: keyboard.topAnchor),
            rightPane.trailingAnchor.constraint(equalTo: root.trailingAnchor),
            rightPane.widthAnchor.constraint(equalToConstant: 280),

            centerContainer.topAnchor.constraint(equalTo: mvtBar.bottomAnchor),
            centerContainer.bottomAnchor.constraint(equalTo: keyboard.topAnchor),
            centerContainer.leadingAnchor.constraint(equalTo: leftPane.trailingAnchor),
            centerContainer.trailingAnchor.constraint(equalTo: rightPane.leadingAnchor),
        ])
    }

    private func styledButton(_ title: String, target: AnyObject?, action: Selector, primary: Bool = false) -> NSButton {
        let b = NSButton(title: title, target: target, action: action)
        b.bezelStyle = .rounded
        b.controlSize = .small
        b.font = NSFont.systemFont(ofSize: 11.5, weight: primary ? .semibold : .medium)
        if primary { b.contentTintColor = StudioPalette.accentGold }
        return b
    }

    private func buildTopTransportBar() -> NSView {
        let bar = NSView()
        bar.wantsLayer = true
        bar.layer?.backgroundColor = StudioPalette.bgHeader.cgColor

        playBtn.title = "▶ Play"
        playBtn.bezelStyle = .rounded
        playBtn.controlSize = .regular
        playBtn.font = NSFont.systemFont(ofSize: 12, weight: .bold)
        playBtn.contentTintColor = StudioPalette.accentGold
        playBtn.target = self
        playBtn.action = #selector(onPlayToggle)

        let stopBtn = styledButton("■ Stop", target: self, action: #selector(onStop))
        loopBtn.title = "⟳ Loop Mvt"
        loopBtn.setButtonType(.pushOnPushOff)
        loopBtn.bezelStyle = .rounded
        loopBtn.controlSize = .small
        loopBtn.target = self
        loopBtn.action = #selector(onLoopToggle)

        let undoBtn = styledButton("↶ Undo", target: self, action: #selector(menuUndo))
        let redoBtn = styledButton("↷ Redo", target: self, action: #selector(menuRedo))

        clockLabel.font = NSFont.monospacedDigitSystemFont(ofSize: 13.5, weight: .semibold)
        clockLabel.textColor = StudioPalette.textPrimary
        bpmKeyLabel.font = NSFont.monospacedSystemFont(ofSize: 12, weight: .semibold)
        bpmKeyLabel.textColor = StudioPalette.accentGold

        viewSeg.segmentCount = 2
        viewSeg.setLabel("Piano Roll", forSegment: 0)
        viewSeg.setLabel("Falling Notes", forSegment: 1)
        viewSeg.selectedSegment = 0
        viewSeg.controlSize = .small
        viewSeg.target = self
        viewSeg.action = #selector(onViewModeChanged)

        let importBtn = styledButton("Import .mid", target: self, action: #selector(menuImportMidi))
        let mergeBtn = styledButton("Merge .mid", target: self, action: #selector(menuMergeMidi))
        let exportBtn = styledButton("Export .mid", target: self, action: #selector(menuExportMidi))
        let wavBtn = styledButton("Bounce WAV", target: self, action: #selector(menuBounceWav), primary: true)

        bankBtn.title = audio.activeBankLabel
        bankBtn.bezelStyle = .rounded
        bankBtn.controlSize = .small
        bankBtn.font = NSFont.systemFont(ofSize: 11, weight: .medium)
        bankBtn.target = self
        bankBtn.action = #selector(onSoundBankClicked)

        let spacer = NSView()
        spacer.setContentHuggingPriority(.defaultLow, for: .horizontal)

        let stack = NSStackView(views: [
            playBtn, stopBtn, loopBtn, undoBtn, redoBtn,
            clockLabel, bpmKeyLabel,
            spacer,
            viewSeg, importBtn, mergeBtn, exportBtn, wavBtn, bankBtn,
        ])
        stack.orientation = .horizontal
        stack.spacing = 8
        stack.edgeInsets = NSEdgeInsets(top: 6, left: 12, bottom: 6, right: 12)
        stack.translatesAutoresizingMaskIntoConstraints = false
        bar.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: bar.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: bar.trailingAnchor),
            stack.topAnchor.constraint(equalTo: bar.topAnchor),
            stack.bottomAnchor.constraint(equalTo: bar.bottomAnchor),
        ])
        return bar
    }

    private func buildMovementAndToolsBar() -> NSView {
        let bar = NSView()
        bar.wantsLayer = true
        bar.layer?.backgroundColor = StudioPalette.bgPanel.cgColor

        let jumpLbl = NSTextField(labelWithString: "MOVEMENTS:")
        jumpLbl.font = NSFont.monospacedSystemFont(ofSize: 10, weight: .bold)
        jumpLbl.textColor = StudioPalette.textMuted

        let labels = [
            "1 · All I Ask (Db)",
            "2 · Nocturne (Db)",
            "3 · Phantom Organ (Dm)",
            "4 · Pt of No Return (Fm)",
            "5 · Grand Reprise",
        ]
        var views: [NSView] = [jumpLbl]
        movementButtons.removeAll()
        for (i, title) in labels.enumerated() {
            let b = styledButton(title, target: self, action: #selector(onMovementJump(_:)))
            b.tag = i + 1
            movementButtons.append(b)
            views.append(b)
        }

        let spacer = NSView()
        spacer.setContentHuggingPriority(.defaultLow, for: .horizontal)
        views.append(spacer)

        toolBtn.title = "Mode: Select (Dbl-Click Adds)"
        toolBtn.bezelStyle = .rounded
        toolBtn.controlSize = .small
        toolBtn.target = self
        toolBtn.action = #selector(onTogglePencilMode)
        views.append(toolBtn)

        let snapPopup = NSPopUpButton(frame: .zero, pullsDown: false)
        snapPopup.controlSize = .small
        snapPopup.addItems(withTitles: ["Snap 1/4", "Snap 1/8", "Snap 1/16", "Snap 1/32", "Snap Off"])
        snapPopup.selectItem(at: 2)
        snapPopup.target = self
        snapPopup.action = #selector(onSnapChanged(_:))
        views.append(snapPopup)

        let revLbl = NSTextField(labelWithString: "Hall:")
        revLbl.font = NSFont.systemFont(ofSize: 10.5, weight: .medium)
        revLbl.textColor = StudioPalette.textMuted
        views.append(revLbl)

        reverbSlider.minValue = 0
        reverbSlider.maxValue = 70
        reverbSlider.doubleValue = 28
        reverbSlider.controlSize = .mini
        reverbSlider.target = self
        reverbSlider.action = #selector(onReverbChanged)
        reverbSlider.widthAnchor.constraint(equalToConstant: 72).isActive = true
        views.append(reverbSlider)

        let stack = NSStackView(views: views)
        stack.orientation = .horizontal
        stack.spacing = 6
        stack.edgeInsets = NSEdgeInsets(top: 4, left: 12, bottom: 4, right: 12)
        stack.translatesAutoresizingMaskIntoConstraints = false
        bar.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: bar.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: bar.trailingAnchor),
            stack.topAnchor.constraint(equalTo: bar.topAnchor),
            stack.bottomAnchor.constraint(equalTo: bar.bottomAnchor),
        ])
        return bar
    }

    private func buildLeftTrackRack() -> NSView {
        let panel = NSView()
        panel.wantsLayer = true
        panel.layer?.backgroundColor = StudioPalette.bgPanel.cgColor

        let hdr = NSTextField(labelWithString: "ORCHESTRA TRACKS")
        hdr.font = NSFont.monospacedSystemFont(ofSize: 10.5, weight: .bold)
        hdr.textColor = StudioPalette.textMuted

        let addBtn = styledButton("+ Track", target: self, action: #selector(onAddTrack))
        let resetBtn = styledButton("Starter", target: self, action: #selector(onResetStarter))

        let topRow = NSStackView(views: [hdr, NSView(), addBtn, resetBtn])
        topRow.orientation = .horizontal
        topRow.spacing = 6
        topRow.edgeInsets = NSEdgeInsets(top: 8, left: 10, bottom: 6, right: 10)
        topRow.translatesAutoresizingMaskIntoConstraints = false

        let scroll = NSScrollView()
        scroll.hasVerticalScroller = true
        scroll.drawsBackground = false
        scroll.translatesAutoresizingMaskIntoConstraints = false

        trackStack.orientation = .vertical
        trackStack.alignment = .leading
        trackStack.spacing = 5
        trackStack.edgeInsets = NSEdgeInsets(top: 6, left: 8, bottom: 12, right: 8)
        trackStack.translatesAutoresizingMaskIntoConstraints = false

        let clip = NSClipView()
        clip.drawsBackground = false
        let doc = FlippedContainerView()
        doc.translatesAutoresizingMaskIntoConstraints = false
        doc.addSubview(trackStack)
        scroll.contentView = clip
        scroll.documentView = doc

        NSLayoutConstraint.activate([
            trackStack.topAnchor.constraint(equalTo: doc.topAnchor),
            trackStack.leadingAnchor.constraint(equalTo: doc.leadingAnchor),
            trackStack.trailingAnchor.constraint(equalTo: doc.trailingAnchor),
            trackStack.bottomAnchor.constraint(equalTo: doc.bottomAnchor),
            doc.widthAnchor.constraint(equalTo: scroll.widthAnchor),
        ])

        panel.addSubview(topRow)
        panel.addSubview(scroll)
        NSLayoutConstraint.activate([
            topRow.topAnchor.constraint(equalTo: panel.topAnchor),
            topRow.leadingAnchor.constraint(equalTo: panel.leadingAnchor),
            topRow.trailingAnchor.constraint(equalTo: panel.trailingAnchor),
            scroll.topAnchor.constraint(equalTo: topRow.bottomAnchor),
            scroll.leadingAnchor.constraint(equalTo: panel.leadingAnchor),
            scroll.trailingAnchor.constraint(equalTo: panel.trailingAnchor),
            scroll.bottomAnchor.constraint(equalTo: panel.bottomAnchor),
        ])
        return panel
    }

    private func buildRightRemixDrawer() -> NSView {
        let panel = NSView()
        panel.wantsLayer = true
        panel.layer?.backgroundColor = StudioPalette.bgPanel.cgColor

        let hdr = NSTextField(labelWithString: "PROGRAMMATIC REMIX")
        hdr.font = NSFont.monospacedSystemFont(ofSize: 10.5, weight: .bold)
        hdr.textColor = StudioPalette.accentGold

        // Mvt 4 Fm <-> Dm A/B Mashup button
        mvt4ToggleBtn.title = "Mvt 4: Transpose Fm → Dm (-3 st)"
        mvt4ToggleBtn.bezelStyle = .rounded
        mvt4ToggleBtn.controlSize = .small
        mvt4ToggleBtn.contentTintColor = StudioPalette.accentCrimson
        mvt4ToggleBtn.target = self
        mvt4ToggleBtn.action = #selector(onToggleMvt4Dm)

        let transRow = NSStackView(views: [
            styledButton("-12", target: self, action: #selector(onQuickTranspose(_:))),
            styledButton("-3", target: self, action: #selector(onQuickTranspose(_:))),
            styledButton("-1", target: self, action: #selector(onQuickTranspose(_:))),
            styledButton("+1", target: self, action: #selector(onQuickTranspose(_:))),
            styledButton("+3", target: self, action: #selector(onQuickTranspose(_:))),
            styledButton("+12", target: self, action: #selector(onQuickTranspose(_:))),
        ])
        let semis = [-12, -3, -1, 1, 3, 12]
        for (idx, v) in transRow.arrangedSubviews.enumerated() { (v as? NSButton)?.tag = semis[idx] }
        transRow.orientation = .horizontal
        transRow.distribution = .fillEqually
        transRow.spacing = 4

        let explodeBtn = styledButton("Explode Selected Track → Strings", target: self, action: #selector(onExplodeChords))
        let dblDownBtn = styledButton("Double Track Octave Down (-12)", target: self, action: #selector(onDoubleDown))
        let dblUpBtn   = styledButton("Double Track Octave Up (+12)", target: self, action: #selector(onDoubleUp))
        let swellBtn   = styledButton("Apply Crescendo Swell", target: self, action: #selector(onApplySwell))
        let humBtn     = styledButton("Humanize", target: self, action: #selector(onHumanize))
        let qtzBtn     = styledButton("Quantize 1/16", target: self, action: #selector(onQuantize))
        let pitBtn     = styledButton("Broadway Pit Seating", target: self, action: #selector(onPitSeating))

        let hqRow = NSStackView(views: [humBtn, qtzBtn])
        hqRow.orientation = .horizontal
        hqRow.distribution = .fillEqually
        hqRow.spacing = 5

        // Live JSON Hot-Reload controls
        let jsonHdr = NSTextField(labelWithString: "LIVE JSON / AGENT SYNC")
        jsonHdr.font = NSFont.monospacedSystemFont(ofSize: 10, weight: .bold)
        jsonHdr.textColor = StudioPalette.textMuted

        let saveJsonBtn = styledButton("Save live.oamusic.json", target: self, action: #selector(onSaveLiveJson))
        hotReloadBtn.title = "Watch & Hot-Reload JSON"
        hotReloadBtn.setButtonType(.switch)
        hotReloadBtn.controlSize = .small
        hotReloadBtn.target = self
        hotReloadBtn.action = #selector(onToggleHotReload)

        // Command Console
        let cmdHdr = NSTextField(labelWithString: "REMIX CONSOLE (Return runs)")
        cmdHdr.font = NSFont.monospacedSystemFont(ofSize: 10, weight: .bold)
        cmdHdr.textColor = StudioPalette.textMuted

        remixInput.placeholderString = "e.g. transpose track=2 bars=25-32 semi=-3"
        remixInput.font = NSFont.monospacedSystemFont(ofSize: 11, weight: .regular)
        remixInput.delegate = self
        remixInput.target = self
        remixInput.action = #selector(onRunRemixCommand)

        let logScroll = NSScrollView()
        logScroll.hasVerticalScroller = true
        logScroll.drawsBackground = true
        logScroll.backgroundColor = StudioPalette.bgCanvas
        remixLog.isEditable = false
        remixLog.backgroundColor = StudioPalette.bgCanvas
        remixLog.textColor = StudioPalette.textMuted
        remixLog.font = NSFont.monospacedSystemFont(ofSize: 10.5, weight: .regular)
        remixLog.string = """
        Coax Remix Engine Ready.
        Tip: Drop any .mid file onto the Piano Roll!
        Commands:
         transpose track=2 bars=25-32 semi=-3
         explode src=0 dst=2,3,4 bars=1-8
         double src=4 dst=1 semi=-12 bars=17-24
         tempo bar=17 bpm=128
         merge ~/song.mid bar=17
        """
        logScroll.documentView = remixLog
        logScroll.heightAnchor.constraint(greaterThanOrEqualToConstant: 140).isActive = true

        let col = NSStackView(views: [
            hdr, mvt4ToggleBtn, transRow, explodeBtn, dblDownBtn, dblUpBtn, swellBtn, hqRow, pitBtn,
            jsonHdr, saveJsonBtn, hotReloadBtn,
            cmdHdr, remixInput, logScroll,
        ])
        col.orientation = .vertical
        col.alignment = .leading
        col.spacing = 7
        col.edgeInsets = NSEdgeInsets(top: 10, left: 10, bottom: 10, right: 10)
        col.translatesAutoresizingMaskIntoConstraints = false
        panel.addSubview(col)

        for v in [mvt4ToggleBtn, transRow, explodeBtn, dblDownBtn, dblUpBtn, swellBtn, hqRow, pitBtn, saveJsonBtn, remixInput, logScroll] {
            v.widthAnchor.constraint(equalTo: col.widthAnchor, constant: -20).isActive = true
        }

        NSLayoutConstraint.activate([
            col.topAnchor.constraint(equalTo: panel.topAnchor),
            col.leadingAnchor.constraint(equalTo: panel.leadingAnchor),
            col.trailingAnchor.constraint(equalTo: panel.trailingAnchor),
            col.bottomAnchor.constraint(equalTo: panel.bottomAnchor),
        ])
        return panel
    }

    private func buildStatusBar() -> NSView {
        let bar = NSView()
        bar.wantsLayer = true
        bar.layer?.backgroundColor = StudioPalette.bgHeader.cgColor

        statusLabel.font = NSFont.monospacedSystemFont(ofSize: 10.5, weight: .regular)
        statusLabel.textColor = StudioPalette.textMuted
        statusLabel.lineBreakMode = .byTruncatingTail
        statusLabel.translatesAutoresizingMaskIntoConstraints = false
        bar.addSubview(statusLabel)

        let hint = NSTextField(labelWithString: "QWERTY: A..K plays piano · Z/X octave · Space play · 1-5 jump · ⌘Scroll zoom · Drop .mid")
        hint.font = NSFont.monospacedSystemFont(ofSize: 10, weight: .regular)
        hint.textColor = StudioPalette.textMuted.withAlphaComponent(0.75)
        hint.translatesAutoresizingMaskIntoConstraints = false
        bar.addSubview(hint)

        NSLayoutConstraint.activate([
            statusLabel.leadingAnchor.constraint(equalTo: bar.leadingAnchor, constant: 10),
            statusLabel.centerYAnchor.constraint(equalTo: bar.centerYAnchor),
            hint.trailingAnchor.constraint(equalTo: bar.trailingAnchor, constant: -10),
            hint.centerYAnchor.constraint(equalTo: bar.centerYAnchor),
            statusLabel.trailingAnchor.constraint(lessThanOrEqualTo: hint.leadingAnchor, constant: -12),
        ])
        return bar
    }

    // MARK: - Track Rack Rendering

    private func rebuildTrackList() {
        for v in trackStack.arrangedSubviews {
            trackStack.removeArrangedSubview(v)
            v.removeFromSuperview()
        }
        let trs = store.tracks()
        for tr in trs {
            let card = NSView()
            card.wantsLayer = true
            let selected = (tr.index == store.selectedTrack)
            card.layer?.backgroundColor = (selected ? StudioPalette.bgCard : StudioPalette.bgCanvas.withAlphaComponent(0.65)).cgColor
            card.layer?.cornerRadius = 6
            card.layer?.borderWidth = selected ? 1.4 : 0.7
            card.layer?.borderColor = (selected ? StudioPalette.sectionColor(tr.section) : StudioPalette.border).cgColor
            card.translatesAutoresizingMaskIntoConstraints = false

            // Color bar
            let bar = NSView()
            bar.wantsLayer = true
            bar.layer?.backgroundColor = StudioPalette.sectionColor(tr.section).cgColor
            bar.layer?.cornerRadius = 2
            bar.translatesAutoresizingMaskIntoConstraints = false

            let nameBtn = NSButton(title: "\(tr.index): \(tr.name)", target: self, action: #selector(onSelectTrack(_:)))
            nameBtn.tag = Int(tr.index)
            nameBtn.isBordered = false
            nameBtn.alignment = .left
            nameBtn.font = NSFont.systemFont(ofSize: 11.5, weight: selected ? .bold : .medium)
            nameBtn.contentTintColor = StudioPalette.textPrimary

            let muteBtn = NSButton(title: "M", target: self, action: #selector(onToggleMute(_:)))
            muteBtn.tag = Int(tr.index)
            muteBtn.bezelStyle = .rounded
            muteBtn.controlSize = .mini
            muteBtn.contentTintColor = tr.mute ? StudioPalette.accentCrimson : StudioPalette.textMuted

            let soloBtn = NSButton(title: "S", target: self, action: #selector(onToggleSolo(_:)))
            soloBtn.tag = Int(tr.index)
            soloBtn.bezelStyle = .rounded
            soloBtn.controlSize = .mini
            soloBtn.contentTintColor = tr.solo ? StudioPalette.accentGold : StudioPalette.textMuted

            let ghostBtn = NSButton(title: "G", target: self, action: #selector(onToggleGhost(_:)))
            ghostBtn.tag = Int(tr.index)
            ghostBtn.bezelStyle = .rounded
            ghostBtn.controlSize = .mini
            ghostBtn.contentTintColor = tr.ghostVisible ? NSColor.systemTeal : StudioPalette.textMuted

            let row1 = NSStackView(views: [nameBtn, NSView(), muteBtn, soloBtn, ghostBtn])
            row1.orientation = .horizontal
            row1.spacing = 3

            // Row 2: GM Instrument Picker + Volume + Transpose preview badge
            let patchPop = NSPopUpButton(frame: .zero, pullsDown: false)
            patchPop.controlSize = .mini
            patchPop.font = NSFont.systemFont(ofSize: 10)
            patchPop.tag = Int(tr.index)
            var selIdx = 0
            for (idx, preset) in StudioPalette.gmPresets.enumerated() {
                patchPop.addItem(withTitle: preset.name)
                if preset.program == tr.gmProgram { selIdx = idx }
            }
            patchPop.selectItem(at: selIdx)
            patchPop.target = self
            patchPop.action = #selector(onPatchChanged(_:))

            let vol = NSSlider(value: Double(tr.gain), minValue: 0, maxValue: 1.4, target: self, action: #selector(onVolumeChanged(_:)))
            vol.tag = Int(tr.index)
            vol.controlSize = .mini
            vol.widthAnchor.constraint(equalToConstant: 62).isActive = true

            let trLbl = tr.transposePreview == 0 ? "0st" : (tr.transposePreview > 0 ? "+\(tr.transposePreview)st" : "\(tr.transposePreview)st")
            let trBtn = NSButton(title: trLbl, target: self, action: #selector(onCycleTrackTranspose(_:)))
            trBtn.tag = Int(tr.index)
            trBtn.bezelStyle = .rounded
            trBtn.controlSize = .mini
            trBtn.contentTintColor = tr.transposePreview == 0 ? StudioPalette.textMuted : StudioPalette.accentGold

            let row2 = NSStackView(views: [patchPop, vol, trBtn])
            row2.orientation = .horizontal
            row2.spacing = 4

            let inner = NSStackView(views: [row1, row2])
            inner.orientation = .vertical
            inner.alignment = .leading
            inner.spacing = 4
            inner.translatesAutoresizingMaskIntoConstraints = false

            card.addSubview(bar)
            card.addSubview(inner)
            NSLayoutConstraint.activate([
                bar.leadingAnchor.constraint(equalTo: card.leadingAnchor, constant: 4),
                bar.topAnchor.constraint(equalTo: card.topAnchor, constant: 5),
                bar.bottomAnchor.constraint(equalTo: card.bottomAnchor, constant: -5),
                bar.widthAnchor.constraint(equalToConstant: 4),

                inner.leadingAnchor.constraint(equalTo: bar.trailingAnchor, constant: 6),
                inner.trailingAnchor.constraint(equalTo: card.trailingAnchor, constant: -6),
                inner.topAnchor.constraint(equalTo: card.topAnchor, constant: 5),
                inner.bottomAnchor.constraint(equalTo: card.bottomAnchor, constant: -5),
                row1.widthAnchor.constraint(equalTo: inner.widthAnchor),
                row2.widthAnchor.constraint(equalTo: inner.widthAnchor),
            ])

            trackStack.addArrangedSubview(card)
            card.widthAnchor.constraint(equalTo: trackStack.widthAnchor, constant: -16).isActive = true
        }
    }

    // MARK: - Wiring, Timers & Keyboard

    private func wireCallbacks() {
        store.onChange = { [weak self] in
            DispatchQueue.main.async {
                guard let self = self else { return }
                self.audio.syncAllTrackPatches()
                self.rebuildTrackList()
                self.pianoRoll.needsDisplay = true
                self.fallingNotes.needsDisplay = true
                self.keyboard.needsDisplay = true
            }
        }
        store.onStatus = { [weak self] msg in
            DispatchQueue.main.async {
                self?.statusLabel.stringValue = msg
                self?.appendRemixLog(msg)
            }
        }
        audio.onBankStatusChanged = { [weak self] label, downloading, _ in
            DispatchQueue.main.async {
                self?.bankBtn.title = downloading ? "Downloading SF2…" : label
                self?.bankBtn.isEnabled = !downloading
            }
        }
        pianoRoll.onMidiFileDropped = { [weak self] url in
            self?.promptDropMidi(url)
        }
    }

    private func startTimers() {
        // 60 Hz UI animation & telemetry refresh
        uiTimer = Timer.scheduledTimer(withTimeInterval: 1.0 / 60.0, repeats: true) { [weak self] _ in
            guard let self = self else { return }
            self.refreshTelemetry()
            if self.audio.isPlaying || !self.store.liveInputPitches.isEmpty {
                self.pianoRoll.needsDisplay = true
                self.fallingNotes.needsDisplay = true
                self.keyboard.needsDisplay = true
            }
            self.hotReloadCounter += 1
            if self.hotReloadCounter >= 30 {
                self.hotReloadCounter = 0
                _ = self.store.pollHotReloadIfNeeded()
            }
        }
    }

    private func refreshTelemetry() {
        let tk = audio.currentTick
        let bar = (tk / 3840) + 1
        let beat = ((tk % 3840) / 960) + 1
        let secs = Double(audio.currentSample) / 48000.0
        let mm = Int(secs) / 60
        let ss = secs.truncatingRemainder(dividingBy: 60.0)
        clockLabel.stringValue = String(format: "%03lld:%02lld  %02d:%04.1f", bar, beat, mm, ss)

        let bpm = store.withLock { Double(ms_bpm_at_tick(store.handle, tk)) / 1000.0 }
        let markers = store.markers()
        let keyStr: String
        if let m = markers.last(where: { $0.tick <= tk }) {
            keyStr = StudioPalette.keyName(tonic: m.tonic, minor: m.minor)
        } else {
            keyStr = "C Maj"
        }
        bpmKeyLabel.stringValue = String(format: "%.0f BPM · %@", bpm, keyStr)
        playBtn.title = audio.isPlaying ? "⏸ Pause" : "▶ Play"
    }

    private func installKeyboardShortcuts() {
        keyMonitorDown = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            guard let self = self else { return event }
            // Don't hijack typing inside text fields!
            if self.window?.firstResponder is NSTextView { return event }
            let mods = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
            if mods.contains(.command) { return event }

            // Space -> Play / Pause
            if event.keyCode == 49 {
                self.onPlayToggle()
                return nil
            }
            // 1..5 -> Jump to Movement 1..5
            let numKeys: [UInt16: Int] = [18: 1, 19: 2, 20: 3, 21: 4, 23: 5]
            if let slot = numKeys[event.keyCode] {
                self.jumpToMovement(slot: slot, autoLoop: mods.contains(.shift))
                return nil
            }
            // Z / X -> Shift QWERTY keyboard octave
            if event.keyCode == 6 {
                self.keyboard.qwertyBaseOctave = max(1, self.keyboard.qwertyBaseOctave - 1)
                self.keyboard.needsDisplay = true
                return nil
            }
            if event.keyCode == 7 {
                self.keyboard.qwertyBaseOctave = min(6, self.keyboard.qwertyBaseOctave + 1)
                self.keyboard.needsDisplay = true
                return nil
            }
            // QWERTY built-in software piano!
            if let semi = self.qwertyMap[event.keyCode], !event.isARepeat {
                let p = UInt8(max(21, min(108, (self.keyboard.qwertyBaseOctave + 1) * 12 + semi)))
                self.activeQwertyKeys[event.keyCode] = p
                self.store.liveInputPitches.insert(p)
                self.audio.auditionNoteOn(track: self.store.selectedTrack, pitch: p, velocity: 100)
                self.keyboard.needsDisplay = true
                self.pianoRoll.needsDisplay = true
                return nil
            }
            return event
        }

        keyMonitorUp = NSEvent.addLocalMonitorForEvents(matching: .keyUp) { [weak self] event in
            guard let self = self else { return event }
            if let p = self.activeQwertyKeys.removeValue(forKey: event.keyCode) {
                self.store.liveInputPitches.remove(p)
                self.audio.auditionNoteOff(track: self.store.selectedTrack, pitch: p)
                self.keyboard.needsDisplay = true
                self.pianoRoll.needsDisplay = true
                return nil
            }
            return event
        }
    }

    // MARK: - Actions

    @objc private func onPlayToggle() { audio.togglePlay(); refreshTelemetry() }
    @objc private func onStop() { audio.stop(); refreshTelemetry(); pianoRoll.needsDisplay = true; fallingNotes.needsDisplay = true; keyboard.needsDisplay = true }

    @objc private func onLoopToggle() {
        store.loopEnabled = (loopBtn.state == .on)
        let r = store.movementRange(slot: store.activeMovement)
        store.loopStartTick = r.startTick
        store.loopEndTick = r.endTick
        pianoRoll.needsDisplay = true
    }

    @objc private func onMovementJump(_ sender: NSButton) {
        jumpToMovement(slot: sender.tag, autoLoop: false)
    }

    private func jumpToMovement(slot: Int, autoLoop: Bool) {
        store.activeMovement = slot
        let r = store.movementRange(slot: slot)
        store.loopStartTick = r.startTick
        store.loopEndTick = r.endTick
        if autoLoop {
            store.loopEnabled = true
            loopBtn.state = .on
        }
        audio.seek(tick: r.startTick)
        pianoRoll.scrollToTick(r.startTick)
        statusLabel.stringValue = "Jumped to Movement \(slot) (Bar \(r.startTick / 3840 + 1)–\(r.endTick / 3840))"
        pianoRoll.needsDisplay = true
        fallingNotes.needsDisplay = true
    }

    @objc private func onViewModeChanged() {
        let cascade = (viewSeg.selectedSegment == 1)
        pianoRoll.isHidden = cascade
        fallingNotes.isHidden = !cascade
        pianoRoll.needsDisplay = true
        fallingNotes.needsDisplay = true
    }

    @objc private func onTogglePencilMode() {
        pianoRoll.pencilMode.toggle()
        toolBtn.title = pianoRoll.pencilMode ? "Mode: Pencil (Click Adds)" : "Mode: Select (Dbl-Click Adds)"
    }

    @objc private func onSnapChanged(_ sender: NSPopUpButton) {
        let snaps: [Int64] = [960, 480, 240, 120, 1]
        store.snapTicks = snaps[max(0, min(snaps.count - 1, sender.indexOfSelectedItem))]
    }

    @objc private func onReverbChanged() {
        audio.setReverbWet(Float(reverbSlider.doubleValue))
    }

    @objc private func onSoundBankClicked() {
        if audio.hasDownloadedMuseScoreBank() {
            audio.selectBestAvailableBank()
            audio.syncAllTrackPatches()
            statusLabel.stringValue = "Active sound bank: \(audio.soundbankFileURL.path)"
        } else {
            audio.downloadMuseScoreSoundBank()
        }
    }

    // Track controls
    @objc private func onSelectTrack(_ sender: NSButton) {
        store.selectedTrack = Int32(sender.tag)
        store.selectedNoteSlots.removeAll()
        rebuildTrackList()
        pianoRoll.needsDisplay = true
    }

    @objc private func onToggleMute(_ sender: NSButton) {
        let idx = Int32(sender.tag)
        if let tr = store.tracks().first(where: { $0.index == idx }) {
            store.withLock { ms_set_track_mute(store.handle, idx, !tr.mute) }
            audio.allNotesOff()
            store.onChange?()
        }
    }

    @objc private func onToggleSolo(_ sender: NSButton) {
        let idx = Int32(sender.tag)
        if let tr = store.tracks().first(where: { $0.index == idx }) {
            store.withLock { ms_set_track_solo(store.handle, idx, !tr.solo) }
            audio.allNotesOff()
            store.onChange?()
        }
    }

    @objc private func onToggleGhost(_ sender: NSButton) {
        let idx = Int32(sender.tag)
        if let tr = store.tracks().first(where: { $0.index == idx }) {
            store.withLock { ms_set_track_ghost(store.handle, idx, !tr.ghostVisible) }
            store.onChange?()
        }
    }

    @objc private func onPatchChanged(_ sender: NSPopUpButton) {
        let idx = Int32(sender.tag)
        let pi = sender.indexOfSelectedItem
        guard pi >= 0 && pi < StudioPalette.gmPresets.count else { return }
        let prog = StudioPalette.gmPresets[pi].program
        store.withLock { ms_set_track_program(store.handle, idx, prog) }
        audio.syncAllTrackPatches()
        statusLabel.stringValue = "Track \(idx) -> \(StudioPalette.gmPresets[pi].name)"
    }

    @objc private func onVolumeChanged(_ sender: NSSlider) {
        let idx = Int32(sender.tag)
        store.withLock { ms_set_track_gain(store.handle, idx, Float(sender.doubleValue)) }
        audio.syncAllTrackPatches()
    }

    @objc private func onCycleTrackTranspose(_ sender: NSButton) {
        let idx = Int32(sender.tag)
        let cycle: [Int8] = [0, -3, -12, 3, 12]
        let cur = store.tracks().first(where: { $0.index == idx })?.transposePreview ?? 0
        let next = cycle[((cycle.firstIndex(of: cur) ?? 0) + 1) % cycle.count]
        store.withLock { ms_set_track_transpose(store.handle, idx, next) }
        store.onChange?()
    }

    @objc private func onAddTrack() {
        let n = store.tracks().count
        let idx = store.withLock { ms_add_track(store.handle, "Custom Track \(n)", 0, 4, UInt8(n % 15), 0.0, 0.30) }
        if idx >= 0 {
            store.selectedTrack = idx
            store.onChange?()
        }
    }

    @objc private func onResetStarter() {
        audio.stop()
        store.withLock { ms_load_starter(store.handle) }
        store.selectedTrack = 0
        store.selectedNoteSlots.removeAll()
        store.mvt4TransposedToDm = false
        mvt4ToggleBtn.title = "Mvt 4: Transpose Fm → Dm (-3 st)"
        store.onChange?()
        statusLabel.stringValue = "Loaded 4-Movement Gothic Opera Symphonic Starter (10 tracks)"
    }

    // Remix Drawer actions
    @objc private func onToggleMvt4Dm() {
        let r = store.movementRange(slot: 4)
        let semi: Int32 = store.mvt4TransposedToDm ? 3 : -3
        let count = store.withLock { ms_remix_transpose(store.handle, -1, r.startTick, r.endTick, semi) }
        store.mvt4TransposedToDm.toggle()
        mvt4ToggleBtn.title = store.mvt4TransposedToDm
            ? "Mvt 4: Restore Dm → Fm (+3 st)"
            : "Mvt 4: Transpose Fm → Dm (-3 st)"
        store.onChange?()
        appendRemixLog("Mvt 4 A/B (\(semi > 0 ? "+3" : "-3") st): shifted \(count) notes across all tracks")
    }

    @objc private func onQuickTranspose(_ sender: NSButton) {
        let semi = Int32(sender.tag)
        let n = store.withLock { ms_remix_transpose(store.handle, store.selectedTrack, 0, 0, semi) }
        store.onChange?()
        appendRemixLog("Transposed track \(store.selectedTrack) by \(semi) st (\(n) notes)")
    }

    @objc private func onExplodeChords() {
        let trCount = store.tracks().count
        let dst: [UInt8] = trCount >= 5 ? [2, 3, 4] : [UInt8(store.selectedTrack)]
        let r = store.movementRange(slot: store.activeMovement)
        let n = store.withLock {
            dst.withUnsafeBufferPointer { ms_remix_explode(store.handle, store.selectedTrack, r.startTick, r.endTick, $0.baseAddress, Int32($0.count)) }
        }
        store.onChange?()
        appendRemixLog("Exploded \(n) chord notes from track \(store.selectedTrack) -> Strings \(dst) in Mvt \(store.activeMovement)")
    }

    @objc private func onDoubleDown() {
        let n = store.withLock { ms_remix_copy_layer(store.handle, store.selectedTrack, store.selectedTrack, 0, 0, 0, -12, 82) }
        store.onChange?()
        appendRemixLog("Doubled track \(store.selectedTrack) an octave down (\(n) notes added)")
    }

    @objc private func onDoubleUp() {
        let n = store.withLock { ms_remix_copy_layer(store.handle, store.selectedTrack, store.selectedTrack, 0, 0, 0, 12, 78) }
        store.onChange?()
        appendRemixLog("Doubled track \(store.selectedTrack) an octave up (\(n) notes added)")
    }

    @objc private func onApplySwell() {
        let r = store.movementRange(slot: store.activeMovement)
        let n = store.withLock { ms_remix_swell(store.handle, store.selectedTrack, r.startTick, r.endTick, 56, 120, 74) }
        store.onChange?()
        appendRemixLog("Applied orchestral swell (56->120->74) to \(n) notes in Mvt \(store.activeMovement)")
    }

    @objc private func onHumanize() {
        let n = store.withLock { ms_remix_humanize(store.handle, store.selectedTrack, 0, 0, 18, 9, UInt32.random(in: 1...99999)) }
        store.onChange?()
        appendRemixLog("Humanized \(n) notes on track \(store.selectedTrack)")
    }

    @objc private func onQuantize() {
        let n = store.withLock { ms_remix_quantize(store.handle, store.selectedTrack, 0, 0, store.snapTicks, 100) }
        store.onChange?()
        appendRemixLog("Quantized \(n) notes on track \(store.selectedTrack) to \(store.snapTicks) ticks")
    }

    @objc private func onPitSeating() {
        store.withLock { ms_remix_pit_seating(store.handle) }
        store.onChange?()
        appendRemixLog("Applied Broadway 29-piece pit-orchestra panning & reverb sends")
    }

    @objc private func onSaveLiveJson() { store.saveLiveJson() }

    @objc private func onToggleHotReload() {
        store.hotReloadEnabled = (hotReloadBtn.state == .on)
        if store.hotReloadEnabled {
            if !FileManager.default.fileExists(atPath: store.liveJsonURL.path) {
                store.saveLiveJson()
            }
            appendRemixLog("Watching \(store.liveJsonURL.path) for programmatic edits…")
        }
    }

    @objc private func onRunRemixCommand() {
        let cmd = remixInput.stringValue
        let reply = store.runRemixCommand(cmd)
        if !reply.isEmpty {
            appendRemixLog("> \(cmd)\n  \(reply)")
            statusLabel.stringValue = reply
        }
        remixInput.stringValue = ""
    }

    private func appendRemixLog(_ line: String) {
        remixLog.string += "\n" + line
        remixLog.scrollToEndOfDocument(nil)
    }

    // Menu & File Operations
    @objc func menuUndo() {
        if store.withLock({ ms_undo(store.handle) }) {
            store.onChange?()
            statusLabel.stringValue = "Undid last edit"
        }
    }

    @objc func menuRedo() {
        if store.withLock({ ms_redo(store.handle) }) {
            store.onChange?()
            statusLabel.stringValue = "Redid edit"
        }
    }

    @objc func menuImportMidi() { pickMidiFile(merge: false) }
    @objc func menuMergeMidi()  { pickMidiFile(merge: true) }

    private func pickMidiFile(merge: Bool) {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [UTType(filenameExtension: "mid") ?? .data, UTType(filenameExtension: "midi") ?? .data]
        panel.allowsMultipleSelection = false
        panel.title = merge ? "Merge MIDI into Movement \(store.activeMovement)" : "Import Standard MIDI File (.mid)"
        if panel.runModal() == .OK, let url = panel.url {
            let offset: Int64 = merge ? store.movementRange(slot: store.activeMovement).startTick : -1
            _ = store.importMidiFile(url, mergeAtTick: offset)
        }
    }

    private func promptDropMidi(_ url: URL) {
        let alert = NSAlert()
        alert.messageText = "Import \(url.lastPathComponent)"
        alert.informativeText = "Merge this MIDI file into Movement \(store.activeMovement) (Bar \(store.movementRange(slot: store.activeMovement).startTick / 3840 + 1)) to mash it up, or replace the whole project?"
        alert.addButton(withTitle: "Merge at Movement \(store.activeMovement)")
        alert.addButton(withTitle: "Replace Project")
        alert.addButton(withTitle: "Cancel")
        let res = alert.runModal()
        if res == .alertFirstButtonReturn {
            let off = store.movementRange(slot: store.activeMovement).startTick
            _ = store.importMidiFile(url, mergeAtTick: off)
        } else if res == .alertSecondButtonReturn {
            _ = store.importMidiFile(url, mergeAtTick: -1)
        }
    }

    @objc func menuExportMidi() {
        let panel = NSSavePanel()
        panel.nameFieldStringValue = "phantom-mashup.mid"
        if panel.runModal() == .OK, let url = panel.url {
            _ = store.exportMidiFile(url)
        }
    }

    @objc func menuBounceWav() {
        let panel = NSSavePanel()
        panel.nameFieldStringValue = "phantom-mashup-48k.wav"
        if panel.runModal() == .OK, let url = panel.url {
            statusLabel.stringValue = "Bouncing 48 kHz stereo WAV offline…"
            audio.bounceOfflineWav(to: url) { [weak self] _, msg in
                self?.statusLabel.stringValue = msg
                self?.appendRemixLog(msg)
            }
        }
    }
}

final class FlippedContainerView: NSView {
    override var isFlipped: Bool { true }
}
