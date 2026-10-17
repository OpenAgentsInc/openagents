import XCTest

/// Captures every screen of the current app for Design Lab
/// (`crates/design-lab`), on the offline demo so nothing signs in. Each shot
/// is `<area>--<screen>.png` in the folder in `DESIGN_LAB_DIR` (pass it as
/// `TEST_RUNNER_DESIGN_LAB_DIR`), or `<area>--<screen>@dark.png` when
/// `DESIGN_LAB_APPEARANCE` is `dark`, with the screen's element tree beside
/// it as `<area>--<screen>[@dark].json`; `scripts/design-lab-capture.sh` runs
/// this once per appearance and files the folder under the commit it was
/// taken from.
final class DesignLabTourTests: XCTestCase {
    override func setUp() {
        // One missing screen shouldn't lose the rest of the tour.
        continueAfterFailure = true
    }

    private func launch(_ args: [String]) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = args
        app.launch()
        return app
    }

    private func shot(_ app: XCUIApplication, _ area: String, _ screen: String) {
        // Let transitions and widgets settle.
        sleep(1)
        let screenshot = app.screenshot()
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = "\(area)--\(screen)"
        attachment.lifetime = .keepAlways
        add(attachment)
        guard let dir = ProcessInfo.processInfo.environment["DESIGN_LAB_DIR"], !dir.isEmpty else { return }
        let folder = URL(fileURLWithPath: dir, isDirectory: true)
        let dark = ProcessInfo.processInfo.environment["DESIGN_LAB_APPEARANCE"] == "dark"
        let file = "\(area)--\(screen)\(dark ? "@dark" : "")"
        try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        try? screenshot.pngRepresentation.write(to: folder.appendingPathComponent(file + ".png"))
        let pixels = Double(screenshot.image.size.width * screenshot.image.scale)
        if let tree = Self.elementTree(app, pixelWidth: pixels) {
            try? tree.write(to: folder.appendingPathComponent(file + ".json"))
        }
    }

    /// Every visible element with its type, identifier, label, value and
    /// frame in points, so Design Lab can select a button or a card on the
    /// picture. `scale` is picture pixels a point.
    private static func elementTree(_ app: XCUIApplication, pixelWidth: Double) -> Data? {
        guard let root = try? app.snapshot() else { return nil }
        let bounds = root.frame
        guard bounds.width > 0 else { return nil }
        var elements: [[String: Any]] = []
        func walk(_ node: XCUIElementSnapshot, _ depth: Int) {
            let f = node.frame
            let type = typeName(node.elementType)
            let named = !node.identifier.isEmpty || !node.label.isEmpty
            if depth > 0, f.width >= 1, f.height >= 1, f.intersects(bounds), type != "window", type != "other" || named {
                var e: [String: Any] = [
                    "type": type, "x": f.minX, "y": f.minY, "width": f.width, "height": f.height,
                ]
                if !node.identifier.isEmpty { e["identifier"] = node.identifier }
                if !node.label.isEmpty { e["label"] = node.label }
                if let value = node.value, !"\(value)".isEmpty { e["value"] = "\(value)" }
                elements.append(e)
            }
            // A keyboard is one element, not its keys.
            if node.elementType == .keyboard { return }
            for child in node.children { walk(child, depth + 1) }
        }
        walk(root, 0)
        let tree: [String: Any] = [
            "scale": pixelWidth / bounds.width, "width": bounds.width, "height": bounds.height,
            "elements": elements,
        ]
        return try? JSONSerialization.data(withJSONObject: tree, options: [.sortedKeys])
    }

    private static func typeName(_ type: XCUIElement.ElementType) -> String {
        switch type {
        case .button: return "button"
        case .staticText: return "text"
        case .textField: return "text-field"
        case .secureTextField: return "secure-text-field"
        case .textView: return "text-view"
        case .searchField: return "search-field"
        case .image: return "image"
        case .icon: return "icon"
        case .cell: return "cell"
        case .switch: return "switch"
        case .toggle: return "toggle"
        case .slider: return "slider"
        case .link: return "link"
        case .scrollView: return "scroll-view"
        case .table: return "table"
        case .collectionView: return "collection-view"
        case .navigationBar: return "navigation-bar"
        case .tabBar: return "tab-bar"
        case .toolbar: return "toolbar"
        case .keyboard: return "keyboard"
        case .alert: return "alert"
        case .sheet: return "sheet"
        case .menu: return "menu"
        case .menuItem: return "menu-item"
        case .segmentedControl: return "segmented-control"
        case .progressIndicator: return "progress"
        case .activityIndicator: return "activity"
        case .pageIndicator: return "page-indicator"
        case .picker: return "picker"
        case .webView: return "web-view"
        case .window: return "window"
        case .other: return "other"
        default: return "element-\(type.rawValue)"
        }
    }

    /// First launch (#11996): the tutorial's pages, then the chat it lands
    /// on. Nothing signs in.
    func testOnboarding() {
        for (step, id) in ["ask", "code", "connect", "backup"].enumerated() {
            let app = launch(["-demo", "-fast", "-route", "onboarding", "-onboarding-step", "\(step)"])
            _ = app.otherElements["onboarding"].waitForExistence(timeout: 15)
            shot(app, "onboarding", "\(step + 1)-\(id)")
            if id == "backup", app.buttons["onboarding-next"].exists {
                app.buttons["onboarding-next"].tap()
                _ = app.otherElements["cloud-chat"].waitForExistence(timeout: 10)
                shot(app, "onboarding", "5-landing-chat")
            }
            app.terminate()
        }
    }

    func testShell() {
        let app = launch(["-demo", "-fast"])
        _ = app.buttons["drawer-menu"].waitForExistence(timeout: 15)

        if app.openDrawer() {
            _ = app.cells["section-pinned"].waitForExistence(timeout: 5)
            shot(app, "drawer", "sessions")
            app.collectionViews.firstMatch.swipeUp()
            shot(app, "drawer", "sessions-scrolled")
        }

        for (route, area, screen) in [
            ("new", "new-session", "new-session"),
            ("more", "settings", "settings"),
            ("search", "drawer", "search"),
        ] {
            let page = launch(["-demo", "-fast", "-route", route])
            _ = page.buttons["drawer-menu"].waitForExistence(timeout: 15)
            sleep(1)
            shot(page, area, screen)
            page.terminate()
        }
    }

    func testCloudChat() {
        let app = launch(["-demo", "-fast", "-route", "cloud"])
        _ = app.otherElements["cloud-chat"].waitForExistence(timeout: 15)
        shot(app, "chat", "cloud-chat")
        let input = app.textViews["composer-input"]
        if input.waitForExistence(timeout: 5) {
            input.tap()
            if app.buttons["Continue"].waitForExistence(timeout: 1) { app.buttons["Continue"].tap() }
            shot(app, "chat", "composer-keyboard")
        }
    }

    /// Every demo session's transcript.
    func testSessions() {
        for id in [
            "chat-veil", "chat-picker", "chat-tabs", "chat-errored", "chat-deploy", "chat-ios-scroll",
            "chat-ios-keyboard", "chat-home", "chat-visuals", "chat-cjk", "chat-blog", "chat-side",
            "chat-oklch", "chat-presence",
        ] {
            let app = launch(["-demo", "-fast", "-route", "chat:\(id)"])
            _ = app.scrollViews["transcript"].waitForExistence(timeout: 15)
            shot(app, "session", String(id.dropFirst("chat-".count)))
            app.terminate()
        }
    }

    func testVoice() {
        let app = launch(["-demo", "-fast", "-voice-preview", "-voice-stage"])
        _ = app.otherElements["voice-stage"].waitForExistence(timeout: 15)
        shot(app, "voice", "voice-stage")
    }
}
