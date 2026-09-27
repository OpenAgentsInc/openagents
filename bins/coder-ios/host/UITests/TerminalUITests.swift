// The terminal screen against a real host and shell on this Mac. Start the
// fixture first (see `serve_a_host_for_a_terminal_run` in crates/coder-mobile),
// then pass its invitation as TEST_RUNNER_CODER_TERMINAL_INVITATION and its
// directory as TEST_RUNNER_CODER_TERMINAL_FIXTURE_DIR. Without them, the test
// is skipped. The world stays synthetic; the Computers surface, drawn in the
// world computer's HUD, uses the live service with loopback relays admitted
// for this launch only.
import XCTest

final class TerminalUITests: XCTestCase {
    private var app: XCUIApplication!

    override func tearDown() {
        XCUIDevice.shared.orientation = .portrait
        super.tearDown()
    }

    func testOpenAShellRunCommandsRotateAndSeeLostAfterRestart() throws {
        let environment = ProcessInfo.processInfo.environment
        guard let invitation = environment["CODER_TERMINAL_INVITATION"], invitation.hasPrefix("coder-host:") else {
            throw XCTSkip("No live host invitation; start the terminal fixture first.")
        }
        let fixture = environment["CODER_TERMINAL_FIXTURE_DIR"].map(URL.init(fileURLWithPath:))
        continueAfterFailure = false
        XCUIDevice.shared.orientation = .portrait
        app = XCUIApplication()
        app.launchArguments = ["--synthetic", "--loopback-test"]
        app.launch()
        app.openWorldComputerHud()
        XCTAssertTrue(app.hudElement("first-run-title").exists)

        // Enroll by pasting the invitation, then open the host.
        app.hudTap("invite-paste")
        app.hudTap("hud-input-type")
        let field = app.descendants(matching: .any)["computers-input"]
        XCTAssertTrue(field.waitForExistence(timeout: 10))
        field.tap()
        field.typeText(invitation)
        app.buttons["computers-submit"].tap()
        XCTAssertTrue(app.waitForHud("notice", containing: "Added Computer", timeout: 30))
        app.hudTap("first-run-continue")
        XCTAssertTrue(app.waitForHud("host-0-status", containing: "Online", timeout: 60))
        app.hudTap("host-0-open")
        app.hudTap("host-terminal")

        // The shell attaches at the grid the HUD's terminal page fits.
        XCTAssertTrue(app.waitForHud("terminal-title", containing: "Terminal on", timeout: 20))
        XCTAssertTrue(app.waitForHud("terminal-status", containing: "Connected", timeout: 60),
                      app.hudLabel("terminal-status"))
        attachScreenshot("Terminal attached")

        // Type a command on the keyboard and read its output from the grid.
        app.hudTap("hud-terminal-keyboard")
        sleep(1)
        type("echo ui-$((6*7))\n")
        XCTAssertTrue(line("ui-42").waitForExistence(timeout: 30))
        attachScreenshot("Command output")

        // Esc, Tab, arrows, and Ctrl come from the accessory row: Ctrl, then
        // `u`, erases the typed line so it never runs.
        type("echo never")
        tapKey("terminal-key-ctrl")
        type("u")
        type("echo ctrl-$((2+3))\n")
        XCTAssertTrue(line("ctrl-5").waitForExistence(timeout: 30))
        XCTAssertFalse(line("never").exists)
        // Ctrl-C interrupts a running command.
        type("sleep 30 && echo slept\n")
        sleep(1)
        tapKey("terminal-key-interrupt")
        type("echo after-$((1+1))\n")
        XCTAssertTrue(line("after-2").waitForExistence(timeout: 15))
        XCTAssertFalse(line("slept").exists)
        // Up recalls the previous command.
        tapKey("terminal-key-up")
        type("\n")
        XCTAssertTrue(app.staticTexts.matching(NSPredicate(format: "label == %@", "after-2"))
            .element(boundBy: 1).waitForExistence(timeout: 15))
        attachScreenshot("Accessory keys")

        // Rotation resizes the shell's terminal.
        type("stty size\n")
        let measured = size(after: nil)
        if measured == nil {
            attachScreenshot("No size")
            let labels = XCTAttachment(string: app.staticTexts.allElementsBoundByIndex.map(\.label).joined(separator: "\n"))
            labels.name = "Labels"
            labels.lifetime = .keepAlways
            add(labels)
        }
        let portrait = try XCTUnwrap(measured)
        XCUIDevice.shared.orientation = .landscapeLeft
        sleep(2)
        type("clear; stty size\n")
        let landscape = try XCTUnwrap(size(after: portrait))
        XCTAssertGreaterThan(landscape.cols, portrait.cols)
        attachScreenshot("Landscape resize \(landscape.rows)x\(landscape.cols)")
        XCUIDevice.shared.orientation = .portrait
        sleep(2)

        // A host restart leaves this terminal lost.
        if let fixture {
            try Data().write(to: fixture.appendingPathComponent("restart"))
            XCTAssertTrue(app.waitForHud("terminal-status", containing: "Lost", timeout: 90))
            attachScreenshot("Lost after host restart")
            tapKey("terminal-reopen")
            XCTAssertTrue(app.waitForHud("terminal-status", containing: "Connected", timeout: 90))
            app.hudTap("hud-terminal-keyboard")
            sleep(1)
            type("echo reopened\n")
            XCTAssertTrue(line("reopened").waitForExistence(timeout: 30))
            attachScreenshot("New terminal after restart")
        }
        tapKey("terminal-close")
        XCTAssertTrue(app.hudElement("host-terminal").exists)
    }

    /// A grid row whose text is exactly `text`.
    private func line(_ text: String) -> XCUIElement {
        app.staticTexts.matching(NSPredicate(format: "label == %@", text)).firstMatch
    }

    /// The `rows cols` line `stty size` printed, other than `previous`.
    private func size(after previous: (rows: Int, cols: Int)?) -> (rows: Int, cols: Int)? {
        let deadline = Date().addingTimeInterval(20)
        let sizes = app.staticTexts.matching(NSPredicate(format: "label MATCHES %@", "^ *[0-9]+ [0-9]+ *$"))
        while Date() < deadline {
            for label in sizes.allElementsBoundByIndex.map(\.label) {
                let parts = label.trimmingCharacters(in: .whitespaces).split(separator: " ")
                guard parts.count == 2, let rows = Int(parts[0]), let cols = Int(parts[1]) else { continue }
                if let previous, previous.rows == rows, previous.cols == cols { continue }
                return (rows, cols)
            }
            sleep(1)
        }
        return nil
    }

    private func type(_ text: String) {
        app.typeText(text)
    }

    private func tapKey(_ key: String) {
        app.hudTap(key)
    }

    private func attachScreenshot(_ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
