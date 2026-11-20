// Firm reason: 6 (AppKit: right-side file viewer/editor pane emulating Zeron's right-pane shell layout, GitHub #13759 & #13760).
import AppKit

private enum FilePaneColors {
    static func hex(_ value: UInt32, alpha: CGFloat = 1.0) -> NSColor {
        NSColor(
            srgbRed: CGFloat((value >> 16) & 0xff) / 255.0,
            green: CGFloat((value >> 8) & 0xff) / 255.0,
            blue: CGFloat(value & 0xff) / 255.0,
            alpha: alpha
        )
    }
    // Coder Noir palette (components/skin/noir.coax)
    static let background = hex(0x0a0a0a)
    static let card = hex(0x0e0e0e)
    static let raised = hex(0x191919)
    static let input = hex(0x1e1e1e)
    static let border = hex(0x2c2c2c)
    static let borderStrong = hex(0x404040)
    static let text = hex(0xededed)
    static let muted = hex(0x818181)
    static let faint = hex(0x656565)
    static let warning = hex(0xe6c15c)
    static let success = hex(0x9fd08a)
    static let danger = hex(0xff4d42)
    static let selection = hex(0xededed, alpha: 0.22)
}

/// Thin dark vertical split view wrapping the Metal canvas (left) and the FilePaneView (right).
final class FilePaneSplitView: NSSplitView {
    override var dividerColor: NSColor { FilePaneColors.border }
    override var dividerThickness: CGFloat { 1.0 }
}

/// Line-number gutter ruler attached to the file editor's NSScrollView.
final class LineNumberRulerView: NSRulerView {
    private weak var textView: NSTextView?
    private let gutterFont = NSFont.monospacedDigitSystemFont(ofSize: 11, weight: .regular)

    init(scrollView: NSScrollView, textView: NSTextView) {
        self.textView = textView
        super.init(scrollView: scrollView, orientation: .verticalRuler)
        self.clientView = textView
        self.ruleThickness = 44
    }

    @available(*, unavailable)
    required init(coder: NSCoder) { fatalError() }

    override var isFlipped: Bool { true }

    func updateThickness() {
        let lineCount = max(1, (textView?.string.components(separatedBy: "\n").count ?? 1))
        let digits = max(3, String(lineCount).count)
        let width = CGFloat(digits) * 8.0 + 18.0
        if abs(ruleThickness - width) > 0.5 {
            ruleThickness = width
        }
        needsDisplay = true
    }

    override func drawHashMarksAndLabels(in rect: NSRect) {
        FilePaneColors.card.setFill()
        rect.fill()

        // Right hairline border separating gutter from code
        FilePaneColors.border.setFill()
        NSRect(x: bounds.maxX - 1, y: rect.minY, width: 1, height: rect.height).fill()

        guard let textView, let layoutManager = textView.layoutManager, let textContainer = textView.textContainer else { return }
        let visibleRect = scrollView?.contentView.bounds ?? bounds
        let glyphRange = layoutManager.glyphRange(forBoundingRect: visibleRect, in: textContainer)
        let nsText = textView.string as NSString
        let textInset = textView.textContainerInset
        let attrs: [NSAttributedString.Key: Any] = [
            .font: gutterFont,
            .foregroundColor: FilePaneColors.faint
        ]

        // Compute line number of first visible character
        let firstChar = layoutManager.characterIndexForGlyph(at: glyphRange.location)
        var lineNumber = 1
        if firstChar > 0 && firstChar <= nsText.length {
            var idx = 0
            while idx < firstChar {
                let r = nsText.lineRange(for: NSRange(location: idx, length: 0))
                if r.upperBound <= firstChar { lineNumber += 1 }
                idx = max(idx + 1, r.upperBound)
            }
        }

        var charIndex = firstChar
        let endChar = min(nsText.length, layoutManager.characterIndexForGlyph(at: NSMaxRange(glyphRange)))

        // Handle empty document
        if nsText.length == 0 {
            drawLineNumber(1, atY: textInset.height - visibleRect.minY, lineHeight: 18, attrs: attrs)
            return
        }

        while charIndex < endChar {
            let lineRange = nsText.lineRange(for: NSRange(location: charIndex, length: 0))
            let lineGlyphRange = layoutManager.glyphRange(forCharacterRange: lineRange, actualCharacterRange: nil)
            if lineGlyphRange.location != NSNotFound {
                let fragRect = layoutManager.lineFragmentRect(forGlyphAt: lineGlyphRange.location, effectiveRange: nil, withoutAdditionalLayout: true)
                let y = fragRect.minY + textInset.height - visibleRect.minY
                drawLineNumber(lineNumber, atY: y, lineHeight: fragRect.height, attrs: attrs)
            }
            lineNumber += 1
            charIndex = max(charIndex + 1, lineRange.upperBound)
        }

        // Trailing newline extra line
        if nsText.length > 0 && nsText.character(at: nsText.length - 1) == 10 {
            let extra = layoutManager.extraLineFragmentRect
            if extra.height > 0 {
                let y = extra.minY + textInset.height - visibleRect.minY
                drawLineNumber(lineNumber, atY: y, lineHeight: extra.height, attrs: attrs)
            }
        }
    }

    private func drawLineNumber(_ number: Int, atY y: CGFloat, lineHeight: CGFloat, attrs: [NSAttributedString.Key: Any]) {
        let label = "\(number)" as NSString
        let size = label.size(withAttributes: attrs)
        let drawY = y + max(0, (lineHeight - size.height) / 2.0)
        let drawX = ruleThickness - size.width - 10.0
        label.draw(at: NSPoint(x: drawX, y: drawY), withAttributes: attrs)
    }
}

/// Compact dark button used in the file pane header.
private final class FilePaneHeaderButton: NSButton {
    var accentHighlighted = false { didSet { updateStyle() } }

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        isBordered = false
        wantsLayer = true
        layer?.cornerRadius = 5
        font = NSFont.systemFont(ofSize: 12, weight: .medium)
        contentTintColor = FilePaneColors.text
        updateStyle()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError() }

    override var isEnabled: Bool {
        didSet { updateStyle() }
    }

    func updateStyle() {
        if accentHighlighted && isEnabled {
            layer?.backgroundColor = FilePaneColors.text.cgColor
            layer?.borderWidth = 0
            contentTintColor = FilePaneColors.background
            attributedTitle = NSAttributedString(string: title, attributes: [
                .font: font ?? NSFont.systemFont(ofSize: 12, weight: .medium),
                .foregroundColor: FilePaneColors.background
            ])
        } else {
            layer?.backgroundColor = FilePaneColors.raised.cgColor
            layer?.borderWidth = 1
            layer?.borderColor = FilePaneColors.border.cgColor
            let fg = isEnabled ? FilePaneColors.text : FilePaneColors.faint
            contentTintColor = fg
            if !title.isEmpty {
                attributedTitle = NSAttributedString(string: title, attributes: [
                    .font: font ?? NSFont.systemFont(ofSize: 12, weight: .medium),
                    .foregroundColor: fg
                ])
            }
        }
    }
}

/// Right-side file viewer and editor pane, styled in Coder Noir.
final class FilePaneView: NSView, NSTextViewDelegate {
    private(set) var currentURL: URL?
    private var savedText: String = ""
    private var feedbackTimer: Timer?

    var isDirty: Bool {
        guard currentURL != nil else { return false }
        return textView.string != savedText
    }

    var onClose: (() -> Void)?
    var onOpenExternally: ((URL) -> Void)?
    var onStateChange: (() -> Void)?

    // Header controls
    private let headerView = NSView()
    private let headerBorder = NSView()
    private let dirtyDot = NSView()
    private let nameLabel = NSTextField(labelWithString: "")
    private let pathLabel = NSTextField(labelWithString: "")
    private let statusLabel = NSTextField(labelWithString: "")
    private let saveButton = FilePaneHeaderButton(frame: .zero)
    private let externalButton = FilePaneHeaderButton(frame: .zero)
    private let closeButton = FilePaneHeaderButton(frame: .zero)

    // Editor
    private let scrollView = NSScrollView()
    let textView = NSTextView()
    private var rulerView: LineNumberRulerView!

    // Footer
    private let footerView = NSView()
    private let footerBorder = NSView()
    private let cursorLabel = NSTextField(labelWithString: "Ln 1, Col 1")
    private let metaLabel = NSTextField(labelWithString: "UTF-8")

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.backgroundColor = FilePaneColors.background.cgColor
        setupHeader()
        setupEditor()
        setupFooter()
        setupLayout()
        updateHeaderState()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError() }

    private func setupHeader() {
        headerView.wantsLayer = true
        headerView.layer?.backgroundColor = FilePaneColors.card.cgColor
        headerView.translatesAutoresizingMaskIntoConstraints = false
        addSubview(headerView)

        headerBorder.wantsLayer = true
        headerBorder.layer?.backgroundColor = FilePaneColors.border.cgColor
        headerBorder.translatesAutoresizingMaskIntoConstraints = false
        headerView.addSubview(headerBorder)

        dirtyDot.wantsLayer = true
        dirtyDot.layer?.backgroundColor = FilePaneColors.warning.cgColor
        dirtyDot.layer?.cornerRadius = 3.5
        dirtyDot.isHidden = true
        dirtyDot.translatesAutoresizingMaskIntoConstraints = false
        headerView.addSubview(dirtyDot)

        nameLabel.font = NSFont.systemFont(ofSize: 13, weight: .semibold)
        nameLabel.textColor = FilePaneColors.text
        nameLabel.lineBreakMode = .byTruncatingMiddle
        nameLabel.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        nameLabel.translatesAutoresizingMaskIntoConstraints = false
        headerView.addSubview(nameLabel)

        pathLabel.font = NSFont.systemFont(ofSize: 11, weight: .regular)
        pathLabel.textColor = FilePaneColors.muted
        pathLabel.lineBreakMode = .byTruncatingHead
        pathLabel.setContentCompressionResistancePriority(.fittingSizeCompression, for: .horizontal)
        pathLabel.translatesAutoresizingMaskIntoConstraints = false
        headerView.addSubview(pathLabel)

        statusLabel.font = NSFont.systemFont(ofSize: 11, weight: .medium)
        statusLabel.textColor = FilePaneColors.muted
        statusLabel.alignment = .right
        statusLabel.isHidden = true
        statusLabel.setContentCompressionResistancePriority(.required, for: .horizontal)
        statusLabel.translatesAutoresizingMaskIntoConstraints = false
        headerView.addSubview(statusLabel)

        saveButton.title = "Save"
        saveButton.toolTip = "Save File (⌘S)"
        saveButton.target = self
        saveButton.action = #selector(savePressed)
        saveButton.translatesAutoresizingMaskIntoConstraints = false
        headerView.addSubview(saveButton)

        externalButton.title = ""
        if let img = NSImage(systemSymbolName: "arrow.up.forward.square", accessibilityDescription: "Open Externally") {
            img.isTemplate = true
            externalButton.image = img
        } else {
            externalButton.title = "Open"
        }
        externalButton.toolTip = "Open Externally"
        externalButton.target = self
        externalButton.action = #selector(openExternallyPressed)
        externalButton.translatesAutoresizingMaskIntoConstraints = false
        headerView.addSubview(externalButton)

        closeButton.title = ""
        if let img = NSImage(systemSymbolName: "xmark", accessibilityDescription: "Close File Pane") {
            img.isTemplate = true
            closeButton.image = img
        } else {
            closeButton.title = "×"
        }
        closeButton.toolTip = "Close File Pane (⇧⌘W)"
        closeButton.target = self
        closeButton.action = #selector(closePressed)
        closeButton.translatesAutoresizingMaskIntoConstraints = false
        headerView.addSubview(closeButton)
    }

    private func setupEditor() {
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = true
        scrollView.autohidesScrollers = true
        scrollView.borderType = .noBorder
        scrollView.drawsBackground = true
        scrollView.backgroundColor = FilePaneColors.background
        scrollView.translatesAutoresizingMaskIntoConstraints = false
        addSubview(scrollView)

        textView.minSize = NSSize(width: 0, height: 0)
        textView.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        textView.isVerticallyResizable = true
        textView.isHorizontallyResizable = true
        textView.autoresizingMask = [.width]
        textView.textContainer?.containerSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        textView.textContainer?.widthTracksTextView = false

        textView.font = NSFont.monospacedSystemFont(ofSize: 13, weight: .regular)
        textView.textColor = FilePaneColors.text
        textView.insertionPointColor = FilePaneColors.text
        textView.backgroundColor = FilePaneColors.background
        textView.drawsBackground = true
        textView.selectedTextAttributes = [
            .backgroundColor: FilePaneColors.selection,
            .foregroundColor: FilePaneColors.text
        ]
        textView.textContainerInset = NSSize(width: 10, height: 10)
        textView.isEditable = true
        textView.isSelectable = true
        textView.isRichText = false
        textView.allowsUndo = true
        textView.usesFindBar = true
        textView.isIncrementalSearchingEnabled = true
        textView.isAutomaticQuoteSubstitutionEnabled = false
        textView.isAutomaticDashSubstitutionEnabled = false
        textView.isAutomaticTextReplacementEnabled = false
        textView.isAutomaticSpellingCorrectionEnabled = false
        textView.isContinuousSpellCheckingEnabled = false
        textView.isGrammarCheckingEnabled = false
        textView.delegate = self

        scrollView.documentView = textView

        rulerView = LineNumberRulerView(scrollView: scrollView, textView: textView)
        scrollView.verticalRulerView = rulerView
        scrollView.hasVerticalRuler = true
        scrollView.rulersVisible = true

        scrollView.contentView.postsBoundsChangedNotifications = true
        NotificationCenter.default.addObserver(self, selector: #selector(scrollDidChange), name: NSView.boundsDidChangeNotification, object: scrollView.contentView)
    }

    private func setupFooter() {
        footerView.wantsLayer = true
        footerView.layer?.backgroundColor = FilePaneColors.card.cgColor
        footerView.translatesAutoresizingMaskIntoConstraints = false
        addSubview(footerView)

        footerBorder.wantsLayer = true
        footerBorder.layer?.backgroundColor = FilePaneColors.border.cgColor
        footerBorder.translatesAutoresizingMaskIntoConstraints = false
        footerView.addSubview(footerBorder)

        cursorLabel.font = NSFont.monospacedDigitSystemFont(ofSize: 11, weight: .regular)
        cursorLabel.textColor = FilePaneColors.muted
        cursorLabel.translatesAutoresizingMaskIntoConstraints = false
        footerView.addSubview(cursorLabel)

        metaLabel.font = NSFont.systemFont(ofSize: 11, weight: .regular)
        metaLabel.textColor = FilePaneColors.faint
        metaLabel.alignment = .right
        metaLabel.translatesAutoresizingMaskIntoConstraints = false
        footerView.addSubview(metaLabel)
    }

    private func setupLayout() {
        NSLayoutConstraint.activate([
            // Header (38pt aligns with Coder's title bar)
            headerView.topAnchor.constraint(equalTo: topAnchor),
            headerView.leadingAnchor.constraint(equalTo: leadingAnchor),
            headerView.trailingAnchor.constraint(equalTo: trailingAnchor),
            headerView.heightAnchor.constraint(equalToConstant: 38),

            headerBorder.leadingAnchor.constraint(equalTo: headerView.leadingAnchor),
            headerBorder.trailingAnchor.constraint(equalTo: headerView.trailingAnchor),
            headerBorder.bottomAnchor.constraint(equalTo: headerView.bottomAnchor),
            headerBorder.heightAnchor.constraint(equalToConstant: 1),

            dirtyDot.leadingAnchor.constraint(equalTo: headerView.leadingAnchor, constant: 12),
            dirtyDot.centerYAnchor.constraint(equalTo: headerView.centerYAnchor),
            dirtyDot.widthAnchor.constraint(equalToConstant: 7),
            dirtyDot.heightAnchor.constraint(equalToConstant: 7),

            nameLabel.leadingAnchor.constraint(equalTo: dirtyDot.trailingAnchor, constant: 7),
            nameLabel.centerYAnchor.constraint(equalTo: headerView.centerYAnchor),

            pathLabel.leadingAnchor.constraint(equalTo: nameLabel.trailingAnchor, constant: 8),
            pathLabel.centerYAnchor.constraint(equalTo: headerView.centerYAnchor),
            pathLabel.trailingAnchor.constraint(lessThanOrEqualTo: statusLabel.leadingAnchor, constant: -8),

            statusLabel.trailingAnchor.constraint(equalTo: saveButton.leadingAnchor, constant: -8),
            statusLabel.centerYAnchor.constraint(equalTo: headerView.centerYAnchor),

            saveButton.trailingAnchor.constraint(equalTo: externalButton.leadingAnchor, constant: -6),
            saveButton.centerYAnchor.constraint(equalTo: headerView.centerYAnchor),
            saveButton.widthAnchor.constraint(greaterThanOrEqualToConstant: 46),
            saveButton.heightAnchor.constraint(equalToConstant: 24),

            externalButton.trailingAnchor.constraint(equalTo: closeButton.leadingAnchor, constant: -6),
            externalButton.centerYAnchor.constraint(equalTo: headerView.centerYAnchor),
            externalButton.widthAnchor.constraint(equalToConstant: 26),
            externalButton.heightAnchor.constraint(equalToConstant: 24),

            closeButton.trailingAnchor.constraint(equalTo: headerView.trailingAnchor, constant: -10),
            closeButton.centerYAnchor.constraint(equalTo: headerView.centerYAnchor),
            closeButton.widthAnchor.constraint(equalToConstant: 26),
            closeButton.heightAnchor.constraint(equalToConstant: 24),

            // Editor
            scrollView.topAnchor.constraint(equalTo: headerView.bottomAnchor),
            scrollView.leadingAnchor.constraint(equalTo: leadingAnchor),
            scrollView.trailingAnchor.constraint(equalTo: trailingAnchor),
            scrollView.bottomAnchor.constraint(equalTo: footerView.topAnchor),

            // Footer
            footerView.leadingAnchor.constraint(equalTo: leadingAnchor),
            footerView.trailingAnchor.constraint(equalTo: trailingAnchor),
            footerView.bottomAnchor.constraint(equalTo: bottomAnchor),
            footerView.heightAnchor.constraint(equalToConstant: 24),

            footerBorder.leadingAnchor.constraint(equalTo: footerView.leadingAnchor),
            footerBorder.trailingAnchor.constraint(equalTo: footerView.trailingAnchor),
            footerBorder.topAnchor.constraint(equalTo: footerView.topAnchor),
            footerBorder.heightAnchor.constraint(equalToConstant: 1),

            cursorLabel.leadingAnchor.constraint(equalTo: footerView.leadingAnchor, constant: 12),
            cursorLabel.centerYAnchor.constraint(equalTo: footerView.centerYAnchor),

            metaLabel.trailingAnchor.constraint(equalTo: footerView.trailingAnchor, constant: -12),
            metaLabel.centerYAnchor.constraint(equalTo: footerView.centerYAnchor),
            metaLabel.leadingAnchor.constraint(greaterThanOrEqualTo: cursorLabel.trailingAnchor, constant: 12)
        ])
    }

    deinit {
        feedbackTimer?.invalidate()
        NotificationCenter.default.removeObserver(self)
    }

    /// Loads a file from disk into the editor. Returns false if the file could not be read as text.
    @discardableResult
    func loadFile(_ url: URL, relativeTo folder: URL? = nil) -> Bool {
        let data: Data
        do {
            data = try Data(contentsOf: url)
        } catch {
            showFeedback("Read failed", color: FilePaneColors.danger)
            return false
        }
        let content: String
        if let utf8 = String(data: data, encoding: .utf8) {
            content = utf8
        } else if let latin1 = String(data: data, encoding: .isoLatin1) {
            content = latin1
        } else {
            showFeedback("Binary file", color: FilePaneColors.danger)
            return false
        }

        currentURL = url
        savedText = content
        textView.string = content
        textView.undoManager?.removeAllActions()
        textView.setSelectedRange(NSRange(location: 0, length: 0))
        textView.scrollToBeginningOfDocument(nil)

        nameLabel.stringValue = url.lastPathComponent
        nameLabel.toolTip = url.path
        let displayPath = abbreviatedPath(url, relativeTo: folder)
        pathLabel.stringValue = displayPath
        pathLabel.toolTip = url.path
        clearFeedback()
        updateHeaderState()
        updateCursorStatus()
        updateMetaStatus(for: url)
        rulerView.updateThickness()
        onStateChange?()
        return true
    }

    /// Writes the current buffer back to disk.
    @discardableResult
    func save() -> Bool {
        guard let url = currentURL else { return false }
        let text = textView.string
        do {
            try Data(text.utf8).write(to: url, options: .atomic)
            savedText = text
            updateHeaderState()
            showFeedback("Saved", color: FilePaneColors.success, autoClearAfter: 2.0)
            onStateChange?()
            return true
        } catch {
            showFeedback("Save failed", color: FilePaneColors.danger)
            let alert = NSAlert(error: error)
            alert.alertStyle = .warning
            alert.runModal()
            return false
        }
    }

    func clearFile() {
        currentURL = nil
        savedText = ""
        textView.string = ""
        nameLabel.stringValue = ""
        pathLabel.stringValue = ""
        clearFeedback()
        updateHeaderState()
        onStateChange?()
    }

    private func abbreviatedPath(_ url: URL, relativeTo folder: URL?) -> String {
        let full = url.standardizedFileURL.path
        if let folder {
            let base = folder.standardizedFileURL.path
            if full.hasPrefix(base + "/") {
                let rel = String(full.dropFirst(base.count + 1))
                let dir = (rel as NSString).deletingLastPathComponent
                return dir.isEmpty ? "" : dir
            }
        }
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        let dir = (full as NSString).deletingLastPathComponent
        if dir == home { return "~" }
        if dir.hasPrefix(home + "/") { return "~/" + dir.dropFirst(home.count + 1) }
        return dir
    }

    private func updateHeaderState() {
        let dirty = isDirty
        dirtyDot.isHidden = !dirty
        saveButton.isEnabled = dirty
        saveButton.accentHighlighted = dirty
        externalButton.isEnabled = currentURL != nil
    }

    private func showFeedback(_ text: String, color: NSColor, autoClearAfter seconds: TimeInterval? = nil) {
        feedbackTimer?.invalidate()
        feedbackTimer = nil
        statusLabel.stringValue = text
        statusLabel.textColor = color
        statusLabel.isHidden = false
        if let seconds {
            feedbackTimer = Timer.scheduledTimer(withTimeInterval: seconds, repeats: false) { [weak self] _ in
                self?.clearFeedback()
            }
        }
    }

    private func clearFeedback() {
        feedbackTimer?.invalidate()
        feedbackTimer = nil
        statusLabel.stringValue = ""
        statusLabel.isHidden = true
    }

    private func updateCursorStatus() {
        let nsText = textView.string as NSString
        let sel = textView.selectedRange()
        let loc = min(sel.location, nsText.length)
        var line = 1
        var col = 1
        if loc > 0 {
            let prefix = nsText.substring(to: loc)
            let lines = prefix.components(separatedBy: "\n")
            line = lines.count
            col = (lines.last?.count ?? 0) + 1
        }
        let totalLines = max(1, textView.string.components(separatedBy: "\n").count)
        cursorLabel.stringValue = "Ln \(line), Col \(col)  ·  \(totalLines) lines"
    }

    private func updateMetaStatus(for url: URL) {
        let ext = url.pathExtension.lowercased()
        let lang: String
        switch ext {
        case "coax": lang = "Coax"
        case "swift": lang = "Swift"
        case "rs": lang = "Rust"
        case "c", "h": lang = "C"
        case "m", "mm": lang = "Objective-C"
        case "ts", "tsx": lang = "TypeScript"
        case "js", "jsx": lang = "JavaScript"
        case "py": lang = "Python"
        case "md": lang = "Markdown"
        case "json": lang = "JSON"
        case "plist": lang = "Property List"
        case "sh", "bash", "zsh": lang = "Shell"
        default: lang = ext.isEmpty ? "Plain Text" : ext.uppercased()
        }
        metaLabel.stringValue = "\(lang)  ·  UTF-8"
    }

    @objc private func scrollDidChange(_ notification: Notification) {
        rulerView.needsDisplay = true
    }

    @objc private func savePressed() { _ = save() }
    @objc private func openExternallyPressed() { if let url = currentURL { onOpenExternally?(url) } }
    @objc private func closePressed() { onClose?() }

    // MARK: - NSTextViewDelegate

    func textDidChange(_ notification: Notification) {
        if isDirty {
            clearFeedback()
        }
        updateHeaderState()
        updateCursorStatus()
        rulerView.updateThickness()
        onStateChange?()
    }

    func textViewDidChangeSelection(_ notification: Notification) {
        updateCursorStatus()
    }
}

// MARK: - CoderWindow File Pane Integration

extension CoderWindow: NSSplitViewDelegate {
    private static let filePaneWidthKey = "filePaneWidth"
    private static let minCanvasWidth: CGFloat = 340
    private static let minFilePaneWidth: CGFloat = 280

    var filePaneOpen: Bool { !filePane.isHidden }
    var canSaveOpenFile: Bool { filePaneOpen && filePane.isDirty }

    /// Opens a file URL in the right-side file viewer/editor pane. Directories fall back to NSWorkspace.
    func openFileInPane(_ url: URL) {
        let fileURL = url.standardizedFileURL
        var isDir: ObjCBool = false
        guard FileManager.default.fileExists(atPath: fileURL.path, isDirectory: &isDir) else { return }
        if isDir.boolValue {
            NSWorkspace.shared.open(fileURL)
            return
        }
        // If switching away from an unsaved file, confirm first.
        if filePaneOpen, filePane.isDirty, filePane.currentURL != fileURL {
            if !confirmDiscardOrSave() { return }
        }
        guard filePane.loadFile(fileURL, relativeTo: folder) else {
            NSWorkspace.shared.open(fileURL)
            return
        }
        if filePane.isHidden {
            filePane.isHidden = false
            splitView.adjustSubviews()
            let total = splitView.bounds.width
            if total > CoderWindow.minCanvasWidth + CoderWindow.minFilePaneWidth {
                let saved = CGFloat(UserDefaults.standard.float(forKey: CoderWindow.filePaneWidthKey))
                let preferred = saved >= CoderWindow.minFilePaneWidth ? saved : min(560, (total * 0.44).rounded())
                let clamped = min(max(preferred, CoderWindow.minFilePaneWidth), total - CoderWindow.minCanvasWidth)
                splitView.setPosition(total - clamped, ofDividerAt: 0)
            }
        }
        window.makeFirstResponder(filePane.textView)
        wake()
    }

    @discardableResult
    func saveOpenFile() -> Bool {
        guard filePaneOpen else { return false }
        return filePane.save()
    }

    func closeFilePane() {
        guard filePaneOpen else { return }
        if filePane.isDirty && !confirmDiscardOrSave() { return }
        filePane.isHidden = true
        filePane.clearFile()
        splitView.adjustSubviews()
        focusComposer()
        wake()
    }

    /// Prompts the user to save or discard unsaved edits. Returns true if it is safe to proceed.
    func confirmDiscardOrSave() -> Bool {
        guard filePane.isDirty else { return true }
        let name = filePane.currentURL?.lastPathComponent ?? "this file"
        let alert = NSAlert()
        alert.alertStyle = .warning
        alert.messageText = "Save changes to \"\(name)\"?"
        alert.informativeText = "Your edits will be lost if you don't save them."
        alert.addButton(withTitle: "Save")
        alert.addButton(withTitle: "Don't Save")
        alert.addButton(withTitle: "Cancel")
        switch alert.runModal() {
        case .alertFirstButtonReturn: return filePane.save()
        case .alertSecondButtonReturn: return true
        default: return false
        }
    }

    // MARK: - NSSplitViewDelegate

    func splitView(_ splitView: NSSplitView, canCollapseSubview subview: NSView) -> Bool {
        false
    }

    func splitView(_ splitView: NSSplitView, constrainMinCoordinate proposedMinimumPosition: CGFloat, ofSubviewAt dividerIndex: Int) -> CGFloat {
        CoderWindow.minCanvasWidth
    }

    func splitView(_ splitView: NSSplitView, constrainMaxCoordinate proposedMaximumPosition: CGFloat, ofSubviewAt dividerIndex: Int) -> CGFloat {
        max(CoderWindow.minCanvasWidth, splitView.bounds.width - CoderWindow.minFilePaneWidth)
    }

    func splitView(_ splitView: NSSplitView, resizeSubviewsWithOldSize oldSize: NSSize) {
        let newSize = splitView.bounds.size
        if filePane.isHidden {
            canvas.frame = NSRect(origin: .zero, size: newSize)
            return
        }
        let divider = splitView.dividerThickness
        let paneW = min(max(filePane.frame.width, CoderWindow.minFilePaneWidth), max(CoderWindow.minFilePaneWidth, newSize.width - CoderWindow.minCanvasWidth - divider))
        let canvasW = max(0, newSize.width - paneW - divider)
        canvas.frame = NSRect(x: 0, y: 0, width: canvasW, height: newSize.height)
        filePane.frame = NSRect(x: canvasW + divider, y: 0, width: paneW, height: newSize.height)
    }

    func splitViewDidResizeSubviews(_ notification: Notification) {
        if !filePane.isHidden && filePane.frame.width >= CoderWindow.minFilePaneWidth {
            UserDefaults.standard.set(Float(filePane.frame.width), forKey: CoderWindow.filePaneWidthKey)
        }
        wake()
    }
}
