// Firm reason: 6 (AppKit/Metal 3D scene viewport over CAMetalLayer for PLE benchmark scenes).
// Reusable 3D PLE Scene visualizer (GitHub #13792, Deliverable 3):
// - PleSceneMetalView: CAMetalLayer-backed Metal renderer for ple-scene.v1.json boxes, cylinders,
//   and clash markers with directional shading, depth buffer, semantic layer colors, orbit/pan/zoom,
//   camera presets (Iso, Plan, Overhead, Under-Floor), and raycast picking.
// - PleScenePanelView: Reusable container with HUD overlays (run summary, camera presets,
//   layer visibility toggles, and selected-element inspector).
import AppKit
import Metal
import ImageIO
import UniformTypeIdentifiers
import QuartzCore
import simd

// Split from SceneView.swift (#13848): PleScenePanelView viewport and HUD overlays.

// MARK: - PleScenePanelView (Viewport + HUD Overlays)

final class PleScenePanelView: NSView {
    let metalView = PleSceneMetalView(frame: .zero)

    private let summaryBox = NSView()
    private let summaryTitle = NSTextField(labelWithString: "3D PLE Scene")
    private let summaryDetail = NSTextField(labelWithString: "")

    private let diffBox = NSView()
    private let diffLabel = NSTextField(labelWithString: "")

    private let presetStack = NSStackView()
    private var presetButtons: [PleCameraPreset: NSButton] = [:]

    private let layerStack = NSStackView()
    private var layerButtons: [PleSemanticLayer: NSButton] = [:]

    private let inspectorBox = NSView()
    private let inspectorTitle = NSTextField(labelWithString: "Inspector")
    private let inspectorBody = NSTextField(wrappingLabelWithString: "Click any element to inspect • Drag to orbit • Shift/Right-drag to pan • Scroll to zoom")

    private let errorBanner = NSTextField(wrappingLabelWithString: "")

    // Stage 4: Viewport tools bar (Wireframe, Grid, Z-Clip slice, PNG Export)
    private let toolsContainer = NSView()
    private let wireButton = NSButton(title: "Wire", target: nil, action: nil)
    private let gridButton = NSButton(title: "Grid", target: nil, action: nil)
    private let clipButton = NSButton(title: "Slice Z", target: nil, action: nil)
    private let clipSlider = NSSlider(value: 10.0, minValue: 0.0, maxValue: 10.0, target: nil, action: nil)
    private let clipValueLabel = NSTextField(labelWithString: "")
    private let exportPngButton = NSButton(title: "PNG", target: nil, action: nil)
    var onStatusMessage: ((String) -> Void)?

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.backgroundColor = NSColor(calibratedRed: 0.085, green: 0.095, blue: 0.115, alpha: 1.0).cgColor
        setupSubviews()
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        wantsLayer = true
        setupSubviews()
    }

    private func styleOverlay(_ view: NSView) {
        view.wantsLayer = true
        view.layer?.backgroundColor = NSColor(calibratedRed: 0.09, green: 0.10, blue: 0.13, alpha: 0.84).cgColor
        view.layer?.cornerRadius = 8
        view.layer?.borderWidth = 1
        view.layer?.borderColor = NSColor(calibratedWhite: 1.0, alpha: 0.12).cgColor
        view.translatesAutoresizingMaskIntoConstraints = false
    }

    private func setupSubviews() {
        metalView.translatesAutoresizingMaskIntoConstraints = false
        addSubview(metalView)
        NSLayoutConstraint.activate([
            metalView.leadingAnchor.constraint(equalTo: leadingAnchor),
            metalView.trailingAnchor.constraint(equalTo: trailingAnchor),
            metalView.topAnchor.constraint(equalTo: topAnchor),
            metalView.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])

        metalView.onSelectElement = { [weak self] elem in
            self?.updateInspector(elem)
        }

        // Top-left: Run summary HUD
        styleOverlay(summaryBox)
        summaryTitle.font = .systemFont(ofSize: 12, weight: .semibold)
        summaryTitle.textColor = .white
        summaryTitle.translatesAutoresizingMaskIntoConstraints = false
        summaryDetail.font = .monospacedSystemFont(ofSize: 11, weight: .regular)
        summaryDetail.textColor = NSColor(calibratedWhite: 0.82, alpha: 1)
        summaryDetail.translatesAutoresizingMaskIntoConstraints = false
        summaryBox.addSubview(summaryTitle)
        summaryBox.addSubview(summaryDetail)
        addSubview(summaryBox)
        styleOverlay(diffBox)
        diffBox.layer?.borderColor = NSColor(calibratedRed: 0.24, green: 0.85, blue: 0.48, alpha: 0.55).cgColor
        diffLabel.font = .monospacedSystemFont(ofSize: 10.5, weight: .medium)
        diffLabel.textColor = NSColor(calibratedRed: 0.82, green: 0.96, blue: 0.86, alpha: 1)
        diffLabel.translatesAutoresizingMaskIntoConstraints = false
        diffBox.addSubview(diffLabel)
        diffBox.isHidden = true
        addSubview(diffBox)

        NSLayoutConstraint.activate([
            diffBox.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
            diffBox.topAnchor.constraint(equalTo: summaryBox.bottomAnchor, constant: 8),
            diffLabel.leadingAnchor.constraint(equalTo: diffBox.leadingAnchor, constant: 10),
            diffLabel.trailingAnchor.constraint(equalTo: diffBox.trailingAnchor, constant: -10),
            diffLabel.topAnchor.constraint(equalTo: diffBox.topAnchor, constant: 6),
            diffLabel.bottomAnchor.constraint(equalTo: diffBox.bottomAnchor, constant: -6),
        ])

        NSLayoutConstraint.activate([
            summaryBox.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
            summaryBox.topAnchor.constraint(equalTo: topAnchor, constant: 12),
            summaryTitle.leadingAnchor.constraint(equalTo: summaryBox.leadingAnchor, constant: 10),
            summaryTitle.trailingAnchor.constraint(equalTo: summaryBox.trailingAnchor, constant: -10),
            summaryTitle.topAnchor.constraint(equalTo: summaryBox.topAnchor, constant: 7),
            summaryDetail.leadingAnchor.constraint(equalTo: summaryBox.leadingAnchor, constant: 10),
            summaryDetail.trailingAnchor.constraint(equalTo: summaryBox.trailingAnchor, constant: -10),
            summaryDetail.topAnchor.constraint(equalTo: summaryTitle.bottomAnchor, constant: 2),
            summaryDetail.bottomAnchor.constraint(equalTo: summaryBox.bottomAnchor, constant: -7),
        ])

        // Top-right: Camera presets
        let presetContainer = NSView()
        styleOverlay(presetContainer)
        presetStack.orientation = .horizontal
        presetStack.spacing = 4
        presetStack.translatesAutoresizingMaskIntoConstraints = false
        for preset in PleCameraPreset.allCases {
            let b = NSButton(title: preset.rawValue, target: self, action: #selector(presetPressed(_:)))
            b.bezelStyle = .recessed
            b.isBordered = false
            b.wantsLayer = true
            b.layer?.cornerRadius = 5
            b.layer?.backgroundColor = NSColor(calibratedWhite: 1.0, alpha: 0.08).cgColor
            b.font = .systemFont(ofSize: 11, weight: .medium)
            b.contentTintColor = .white
            b.attributedTitle = NSAttributedString(string: " \(preset.rawValue) ", attributes: [
                .foregroundColor: NSColor.white,
                .font: NSFont.systemFont(ofSize: 11, weight: .medium),
            ])
            presetButtons[preset] = b
            presetStack.addArrangedSubview(b)
        }
        presetContainer.addSubview(presetStack)
        addSubview(presetContainer)
        NSLayoutConstraint.activate([
            presetContainer.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -12),
            presetContainer.topAnchor.constraint(equalTo: topAnchor, constant: 12),
            presetStack.leadingAnchor.constraint(equalTo: presetContainer.leadingAnchor, constant: 6),
            presetStack.trailingAnchor.constraint(equalTo: presetContainer.trailingAnchor, constant: -6),
            presetStack.topAnchor.constraint(equalTo: presetContainer.topAnchor, constant: 5),
            presetStack.bottomAnchor.constraint(equalTo: presetContainer.bottomAnchor, constant: -5),
        ])

        // Top-right (below presets): Stage 4 Viewport Tools (Wireframe, Grid, Z-Slice, PNG Export)
        styleOverlay(toolsContainer)
        let toolsStack = NSStackView()
        toolsStack.orientation = .horizontal
        toolsStack.alignment = .centerY
        toolsStack.spacing = 5
        toolsStack.translatesAutoresizingMaskIntoConstraints = false

        func configureToolButton(_ b: NSButton, title: String, action: Selector, tip: String) {
            b.title = title
            b.target = self
            b.action = action
            b.toolTip = tip
            b.bezelStyle = .recessed
            b.isBordered = false
            b.wantsLayer = true
            b.layer?.cornerRadius = 5
            b.layer?.borderWidth = 1
            b.font = .systemFont(ofSize: 10.5, weight: .medium)
        }

        configureToolButton(wireButton, title: " Wire ", action: #selector(wirePressed), tip: "Toggle wireframe rendering")
        configureToolButton(gridButton, title: " Grid ", action: #selector(gridPressed), tip: "Toggle ground reference grid")
        configureToolButton(clipButton, title: " Slice Z ", action: #selector(clipToggled), tip: "Toggle horizontal Z section cut plane")
        configureToolButton(exportPngButton, title: " PNG ", action: #selector(exportPngPressed), tip: "Export 3D viewport snapshot as PNG")

        clipSlider.target = self
        clipSlider.action = #selector(clipSliderChanged)
        clipSlider.isContinuous = true
        clipSlider.controlSize = .mini
        clipSlider.translatesAutoresizingMaskIntoConstraints = false
        clipSlider.widthAnchor.constraint(equalToConstant: 82).isActive = true
        clipSlider.isHidden = true

        clipValueLabel.font = .monospacedSystemFont(ofSize: 10, weight: .medium)
        clipValueLabel.textColor = NSColor(calibratedRed: 0.72, green: 0.88, blue: 1.0, alpha: 1.0)
        clipValueLabel.isHidden = true

        toolsStack.addArrangedSubview(wireButton)
        toolsStack.addArrangedSubview(gridButton)
        toolsStack.addArrangedSubview(clipButton)
        toolsStack.addArrangedSubview(clipSlider)
        toolsStack.addArrangedSubview(clipValueLabel)
        toolsStack.addArrangedSubview(exportPngButton)
        toolsContainer.addSubview(toolsStack)
        addSubview(toolsContainer)

        NSLayoutConstraint.activate([
            toolsContainer.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -12),
            toolsContainer.topAnchor.constraint(equalTo: presetContainer.bottomAnchor, constant: 6),
            toolsStack.leadingAnchor.constraint(equalTo: toolsContainer.leadingAnchor, constant: 6),
            toolsStack.trailingAnchor.constraint(equalTo: toolsContainer.trailingAnchor, constant: -6),
            toolsStack.topAnchor.constraint(equalTo: toolsContainer.topAnchor, constant: 4),
            toolsStack.bottomAnchor.constraint(equalTo: toolsContainer.bottomAnchor, constant: -4),
        ])

        refreshViewportToolButtons()
        metalView.onSceneBoundsChanged = { [weak self] bounds in
            self?.syncClipBounds(bounds)
        }

        // Bottom-left: Layer visibility toggles
        let layerContainer = NSView()
        styleOverlay(layerContainer)
        layerStack.orientation = .horizontal
        layerStack.spacing = 4
        layerStack.translatesAutoresizingMaskIntoConstraints = false
        for sem in PleSemanticLayer.allCases {
            let b = NSButton(title: sem.displayName, target: self, action: #selector(layerToggled(_:)))
            b.isBordered = false
            b.wantsLayer = true
            b.layer?.cornerRadius = 5
            b.tag = PleSemanticLayer.allCases.firstIndex(of: sem) ?? 0
            layerButtons[sem] = b
            layerStack.addArrangedSubview(b)
            refreshLayerButton(sem)
        }
        layerContainer.addSubview(layerStack)
        addSubview(layerContainer)
        NSLayoutConstraint.activate([
            layerContainer.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
            layerContainer.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -12),
            layerStack.leadingAnchor.constraint(equalTo: layerContainer.leadingAnchor, constant: 6),
            layerStack.trailingAnchor.constraint(equalTo: layerContainer.trailingAnchor, constant: -6),
            layerStack.topAnchor.constraint(equalTo: layerContainer.topAnchor, constant: 5),
            layerStack.bottomAnchor.constraint(equalTo: layerContainer.bottomAnchor, constant: -5),
        ])

        // Bottom-right: Selected element inspector
        styleOverlay(inspectorBox)
        inspectorTitle.font = .systemFont(ofSize: 11, weight: .semibold)
        inspectorTitle.textColor = .white
        inspectorTitle.translatesAutoresizingMaskIntoConstraints = false
        inspectorBody.font = .monospacedSystemFont(ofSize: 10.5, weight: .regular)
        inspectorBody.textColor = NSColor(calibratedWhite: 0.82, alpha: 1)
        inspectorBody.maximumNumberOfLines = 6
        inspectorBody.translatesAutoresizingMaskIntoConstraints = false
        inspectorBox.addSubview(inspectorTitle)
        inspectorBox.addSubview(inspectorBody)
        addSubview(inspectorBox)
        NSLayoutConstraint.activate([
            inspectorBox.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -12),
            inspectorBox.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -46),
            inspectorBox.widthAnchor.constraint(lessThanOrEqualToConstant: 290),
            inspectorTitle.leadingAnchor.constraint(equalTo: inspectorBox.leadingAnchor, constant: 10),
            inspectorTitle.trailingAnchor.constraint(equalTo: inspectorBox.trailingAnchor, constant: -10),
            inspectorTitle.topAnchor.constraint(equalTo: inspectorBox.topAnchor, constant: 7),
            inspectorBody.leadingAnchor.constraint(equalTo: inspectorBox.leadingAnchor, constant: 10),
            inspectorBody.trailingAnchor.constraint(equalTo: inspectorBox.trailingAnchor, constant: -10),
            inspectorBody.topAnchor.constraint(equalTo: inspectorTitle.bottomAnchor, constant: 3),
            inspectorBody.bottomAnchor.constraint(equalTo: inspectorBox.bottomAnchor, constant: -7),
        ])

        // Error banner (hidden unless a load fails)
        errorBanner.isHidden = true
        errorBanner.font = .systemFont(ofSize: 12, weight: .medium)
        errorBanner.textColor = NSColor(calibratedRed: 1.0, green: 0.72, blue: 0.72, alpha: 1)
        errorBanner.alignment = .center
        errorBanner.translatesAutoresizingMaskIntoConstraints = false
        addSubview(errorBanner)
        NSLayoutConstraint.activate([
            errorBanner.centerXAnchor.constraint(equalTo: centerXAnchor),
            errorBanner.centerYAnchor.constraint(equalTo: centerYAnchor),
            errorBanner.widthAnchor.constraint(lessThanOrEqualTo: widthAnchor, constant: -40),
        ])
    }

    private func refreshLayerButton(_ sem: PleSemanticLayer) {
        guard let b = layerButtons[sem] else { return }
        let on = metalView.visibleLayers.contains(sem)
        b.layer?.backgroundColor = on
            ? sem.nsColor.withAlphaComponent(0.28).cgColor
            : NSColor(calibratedWhite: 1.0, alpha: 0.04).cgColor
        b.layer?.borderWidth = 1
        b.layer?.borderColor = on
            ? sem.nsColor.withAlphaComponent(0.85).cgColor
            : NSColor(calibratedWhite: 1.0, alpha: 0.12).cgColor
        let title = " ● \(sem.displayName) "
        let attr = NSMutableAttributedString(string: title, attributes: [
            .font: NSFont.systemFont(ofSize: 10.5, weight: .medium),
            .foregroundColor: on ? NSColor.white : NSColor(calibratedWhite: 0.55, alpha: 1),
        ])
        attr.addAttribute(.foregroundColor, value: on ? sem.nsColor : NSColor(calibratedWhite: 0.4, alpha: 1), range: NSRange(location: 1, length: 1))
        b.attributedTitle = attr
    }

    /// A camera preset, as its button does (a program's request).
    func applyCamera(_ preset: PleCameraPreset) { metalView.applyPreset(preset) }

    /// Shows or hides a layer, as its button does, and keeps the button in step.
    func setLayer(_ layer: PleSemanticLayer, visible: Bool) {
        metalView.setLayerVisible(layer, visible: visible)
        refreshLayerButton(layer)
    }

    /// Selects an element and turns to it; false when the scene has none by that id.
    func selectElement(id: String) -> Bool { metalView.selectElement(id: id) }

    @objc private func presetPressed(_ sender: NSButton) {
        for (preset, btn) in presetButtons where btn === sender {
            metalView.applyPreset(preset)
            break
        }
    }

    @objc private func layerToggled(_ sender: NSButton) {
        let all = PleSemanticLayer.allCases
        guard sender.tag >= 0 && sender.tag < all.count else { return }
        let sem = all[sender.tag]
        let nowOn = !metalView.visibleLayers.contains(sem)
        metalView.setLayerVisible(sem, visible: nowOn)
        refreshLayerButton(sem)
    }

    /// Makes this the live 3D card in the transcript (page/shows.coax, #13812): the scene and its
    /// summary, without the tool bars, the layer list or the inspector; the wheel scrolls the chat.
    /// Once made compact a panel stays so.
    func makeCompact() {
        for v in subviews where v !== metalView && v !== summaryBox && v !== errorBanner { v.isHidden = true }
        metalView.passesScroll = true
        layer?.cornerRadius = 10
        layer?.masksToBounds = true
    }

    /// What the page drew over this view (its menus, popovers, dialogs and toasts: coder_page_over_rects),
    /// in this view's coordinates: cut out of it, so the page's drawing under it shows there, and
    /// presses there go to the page (#13817).
    private(set) var holes: [NSRect] = []

    func setHoles(_ rects: [NSRect]) {
        let cut = rects.map { $0.intersection(bounds) }.filter { !$0.isEmpty }
        if cut == holes { return }
        holes = cut
        guard let layer = layer else { return }
        if cut.isEmpty { layer.mask = nil; return }
        let mask = (layer.mask as? CAShapeLayer) ?? CAShapeLayer()
        let path = CGMutablePath()
        path.addRect(bounds)
        for r in cut { path.addRect(r) }
        mask.frame = bounds
        mask.fillRule = .evenOdd
        mask.path = path
        layer.mask = mask
    }

    override func hitTest(_ point: NSPoint) -> NSView? {
        if !holes.isEmpty {
            let p = convert(point, from: superview)
            if holes.contains(where: { $0.contains(p) }) { return nil }
        }
        return super.hitTest(point)
    }

    func loadScene(_ scene: PleScene, preserveView: Bool = false) {
        errorBanner.isHidden = true
        metalView.setScene(scene, preserveView: preserveView)
        summaryTitle.stringValue = scene.task ?? "PLE 3D Scene"
        var parts: [String] = []
        if let score = scene.run?.score { parts.append(String(format: "score %.2f", score)) }
        if let g = scene.run?.gateSummary { parts.append("gates \(g.passed)/\(g.total)") }
        if let len = scene.run?.total_length_m { parts.append(String(format: "%.1fm", len)) }
        parts.append("\(scene.allElements.count) els")
        let clashes = scene.totalClashCount
        if clashes > 0 { parts.append("\(clashes) clashes") }
        summaryDetail.stringValue = parts.joined(separator: " • ")
        for sem in PleSemanticLayer.allCases { refreshLayerButton(sem) }
        if !preserveView {
            updateInspector(nil)
        }
    }

    /// Focuses the 3D camera smoothly onto a clash and highlights it in the inspector strip.
    func focusClash(_ clash: PleResolvedClash) {
        if !metalView.visibleLayers.contains(.clashes) {
            metalView.setLayerVisible(.clashes, visible: true)
            refreshLayerButton(.clashes)
        }
        metalView.focusOn(point: clash.point, distance: 4.5, highlightElementId: clash.elementId, animated: true)
        inspectorTitle.stringValue = "CLASH: \(clash.pairLabel)"
        var lines: [String] = []
        if let pen = clash.penetrationMM {
            lines.append(String(format: "penetration: %.1f mm", pen))
        } else if let sev = clash.severity {
            lines.append("severity: \(sev)")
        }
        lines.append(String(format: "point: (%.2f, %.2f, %.2f) m", clash.point.x, clash.point.y, clash.point.z))
        inspectorBody.stringValue = lines.joined(separator: "  •  ")
    }

    /// Updates the visual diff overlay and diff summary HUD.
    func setDiff(_ diff: PleSceneDiff?) {
        metalView.setDiff(diff)
        if let diff = diff {
            diffLabel.stringValue = "DIFF  " + diff.summaryText() + "   [green=new, orange=moved, red=removed]"
            diffBox.isHidden = false
        } else {
            diffBox.isHidden = true
            diffLabel.stringValue = ""
        }
    }

    func loadScene(from url: URL) throws {
        let scene = try PleScene.load(from: url)
        loadScene(scene)
    }

    private func syncClipBounds(_ bounds: PleSceneBounds) {
        let minZ = Double(bounds.minVec.z)
        let maxZ = Double(max(bounds.maxVec.z + 0.05, bounds.minVec.z + 0.5))
        clipSlider.minValue = minZ
        clipSlider.maxValue = maxZ
        if !metalView.clipZEnabled {
            clipSlider.doubleValue = maxZ
            metalView.clipZMax = Float(maxZ)
        } else {
            let clamped = min(max(Double(metalView.clipZMax), minZ), maxZ)
            clipSlider.doubleValue = clamped
            metalView.clipZMax = Float(clamped)
        }
        clipValueLabel.stringValue = String(format: "Z≤%.1fm", clipSlider.doubleValue)
    }

    private func refreshViewportToolButtons() {
        func styleToggle(_ b: NSButton, title: String, active: Bool, accent: NSColor) {
            b.layer?.backgroundColor = active
                ? accent.withAlphaComponent(0.30).cgColor
                : NSColor(calibratedWhite: 1.0, alpha: 0.07).cgColor
            b.layer?.borderColor = active
                ? accent.withAlphaComponent(0.85).cgColor
                : NSColor(calibratedWhite: 1.0, alpha: 0.14).cgColor
            b.attributedTitle = NSAttributedString(string: title, attributes: [
                .foregroundColor: active ? NSColor.white : NSColor(calibratedWhite: 0.78, alpha: 1.0),
                .font: NSFont.systemFont(ofSize: 10.5, weight: active ? .semibold : .medium),
            ])
        }
        let blue = NSColor(calibratedRed: 0.30, green: 0.65, blue: 1.0, alpha: 1.0)
        let teal = NSColor(calibratedRed: 0.24, green: 0.85, blue: 0.68, alpha: 1.0)
        styleToggle(wireButton, title: " Wire ", active: metalView.wireframeMode, accent: blue)
        styleToggle(gridButton, title: " Grid ", active: metalView.showGrid, accent: blue)
        styleToggle(clipButton, title: " Slice Z ", active: metalView.clipZEnabled, accent: teal)
        styleToggle(exportPngButton, title: " PNG ", active: false, accent: blue)
        clipSlider.isHidden = !metalView.clipZEnabled
        clipValueLabel.isHidden = !metalView.clipZEnabled
        clipValueLabel.stringValue = String(format: "Z≤%.1fm", clipSlider.doubleValue)
    }

    @objc private func wirePressed() {
        metalView.wireframeMode.toggle()
        refreshViewportToolButtons()
    }

    @objc private func gridPressed() {
        metalView.showGrid.toggle()
        refreshViewportToolButtons()
    }

    @objc private func clipToggled() {
        metalView.clipZEnabled.toggle()
        if metalView.clipZEnabled && clipSlider.doubleValue >= clipSlider.maxValue - 0.01 {
            let mid = (clipSlider.minValue + clipSlider.maxValue) * 0.65
            clipSlider.doubleValue = mid
            metalView.clipZMax = Float(mid)
        }
        refreshViewportToolButtons()
    }

    @objc private func clipSliderChanged() {
        metalView.clipZMax = Float(clipSlider.doubleValue)
        clipValueLabel.stringValue = String(format: "Z≤%.1fm", clipSlider.doubleValue)
    }

    @objc private func exportPngPressed() {
        guard let pngData = metalView.renderPNGData(width: 1920, height: 1080) else {
            onStatusMessage?("PNG export failed: Metal viewport not ready")
            return
        }
        let panel = NSSavePanel()
        panel.title = "Export 3D PLE Viewport Snapshot"
        panel.allowedContentTypes = [.png]
        panel.nameFieldStringValue = "ple-viewport.png"
        panel.canCreateDirectories = true
        let finish: (NSApplication.ModalResponse) -> Void = { [weak self] resp in
            guard resp == .OK, let url = panel.url else { return }
            do {
                try pngData.write(to: url, options: .atomic)
                self?.onStatusMessage?("Saved PNG snapshot: \(url.lastPathComponent)")
            } catch {
                self?.onStatusMessage?("PNG save failed: \(error.localizedDescription)")
            }
        }
        if let win = window {
            panel.beginSheetModal(for: win, completionHandler: finish)
        } else {
            finish(panel.runModal())
        }
    }

    func showError(_ message: String) {
        errorBanner.stringValue = message
        errorBanner.isHidden = false
    }

    private func updateInspector(_ elem: PleSceneElement?) {
        guard let elem = elem else {
            inspectorTitle.stringValue = "Inspector"
            inspectorBody.stringValue = "Click any element to inspect • Drag to orbit • Shift/Right-drag to pan • Scroll to zoom"
            return
        }
        inspectorTitle.stringValue = "\(elem.id)  [\(elem.semanticLayer.displayName)]"
        var lines: [String] = []
        if let cls = elem.ifc_class { lines.append("class: \(cls) (\(elem.shape))") }
        let c = elem.centerVec, s = elem.sizeVec
        lines.append(String(format: "pos: (%.2f, %.2f, %.2f)  size: %.2f×%.2f×%.2f", c.x, c.y, c.z, s.x, s.y, s.z))
        if let meta = elem.meta, !meta.isEmpty {
            let pairs = meta.keys.sorted().prefix(4).map { "\($0): \(meta[$0]!)" }
            lines.append(pairs.joined(separator: " • "))
        }
        inspectorBody.stringValue = lines.joined(separator: "\n")
    }
}

