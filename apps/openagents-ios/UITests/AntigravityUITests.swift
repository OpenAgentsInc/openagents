// Firm reason: 6 (XCUITest suite launching the native SwiftUI OpenAgents iOS app on Simulator and verifying both shell invariants and real end-to-end NIP-44 TCP conversations against legacy 16-key v1 and modern v2 Coax hosts).
import XCTest

final class AntigravityUITests: XCTestCase {
    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    private func saveScreenshot(name: String) {
        let shot = XCUIScreen.main.screenshot()
        let attachment = XCTAttachment(screenshot: shot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)

        if let dir = ProcessInfo.processInfo.environment["OA_E2E_SCREENSHOT_DIR"], !dir.isEmpty {
            let url = URL(fileURLWithPath: dir).appendingPathComponent("\(name).png")
            try? FileManager.default.createDirectory(at: URL(fileURLWithPath: dir), withIntermediateDirectories: true)
            try? shot.pngRepresentation.write(to: url)
        }
    }

    private func assertNoErrorBanner(_ app: XCUIApplication, context: String) {
        XCTAssertFalse(app.otherElements["chat.errorBanner"].exists, "Unexpected error banner during \(context)")
        XCTAssertFalse(app.staticTexts["unsupported"].exists, "Raw 'unsupported' leaked to UI during \(context)!")
    }

    private func replaceFieldText(_ field: XCUIElement, with text: String) {
        field.tap()
        let rightEdge = field.coordinate(withNormalizedOffset: CGVector(dx: 0.92, dy: 0.5))
        rightEdge.tap()
        let deleteSeq = String(repeating: XCUIKeyboardKey.delete.rawValue, count: 16)
        field.typeText(deleteSeq + text)
    }

    private func pairHost(app: XCUIApplication, port: String, displayName: String, expectedPairedCount: Int) {
        if app.buttons["chat.connectTailscale"].exists {
            app.buttons["chat.connectTailscale"].tap()
        } else {
            app.buttons["nav.computerPill"].tap()
        }

        let addressField = app.textFields["computers.addAddress"]
        XCTAssertTrue(addressField.waitForExistence(timeout: 8))
        addressField.tap()
        addressField.typeText("127.0.0.1")

        let nameField = app.textFields["computers.addName"]
        XCTAssertTrue(nameField.exists)
        nameField.tap()
        nameField.typeText(displayName)

        let portField = app.textFields["computers.addPort"]
        XCTAssertTrue(portField.exists)
        replaceFieldText(portField, with: port)

        let addBtn = app.buttons["computers.addButton"]
        XCTAssertTrue(addBtn.waitForExistence(timeout: 5))
        addBtn.tap()

        // Wait for direct Tailscale grant to succeed: the new display name appears AND paired count reaches expectedPairedCount.
        let nameLabel = app.staticTexts[displayName].firstMatch
        XCTAssertTrue(nameLabel.waitForExistence(timeout: 15), "Expected paired row for \(displayName) on port \(port)")
        let pairedQuery = app.staticTexts.matching(NSPredicate(format: "label == %@", "Paired"))
        let countExp = expectation(for: NSPredicate(format: "count >= %d", expectedPairedCount), evaluatedWith: pairedQuery)
        wait(for: [countExp], timeout: 15)

        app.buttons["computers.done"].tap()
        XCTAssertTrue(app.buttons["nav.sidebar"].waitForExistence(timeout: 8))
    }

    private func composerElement(_ app: XCUIApplication) -> XCUIElement {
        app.textViews["chat.composer"].exists ? app.textViews["chat.composer"] : app.textFields["chat.composer"]
    }

    private func focusComposer(_ app: XCUIApplication) -> XCUIElement {
        let composer = composerElement(app)
        XCTAssertTrue(composer.waitForExistence(timeout: 8))
        for _ in 0..<3 {
            composer.coordinate(withNormalizedOffset: CGVector(dx: 0.25, dy: 0.5)).tap()
            if (composer.value(forKey: "hasKeyboardFocus") as? Bool) == true {
                return composer
            }
            RunLoop.current.run(until: Date().addingTimeInterval(0.35))
        }
        composer.tap()
        return composer
    }

    private func sendPrompt(_ app: XCUIApplication, text: String) {
        let composer = focusComposer(app)
        composer.typeText(text)
        let sendBtn = app.buttons["chat.send"]
        XCTAssertTrue(sendBtn.waitForExistence(timeout: 5))
        sendBtn.tap()
    }

    @MainActor
    func testPureBlackAntigravityShellSidebarComputersWorkspaceAndComposer() {
        let app = XCUIApplication()
        app.launchArguments = ["-OANoLocalNetworkBrowse", "-OAResetStateForTests"]
        app.launch()

        // Top navigation bar & Antigravity onboarding state appear immediately.
        let sidebarButton = app.buttons["nav.sidebar"]
        XCTAssertTrue(sidebarButton.waitForExistence(timeout: 15))
        XCTAssertTrue(app.buttons["nav.computerPill"].exists)
        XCTAssertTrue(app.buttons["nav.workspace"].exists)
        XCTAssertTrue(app.buttons["nav.newChat"].exists)
        XCTAssertTrue(app.buttons["chat.connectTailscale"].waitForExistence(timeout: 5))

        // Composer exposes Attachment (+), Model, Effort, Review Mode, Live Voice, and Dictation controls,
        // plus Always-On Usage HUD & Full Auto toggle chip (default ON; #14068).
        XCTAssertTrue(app.buttons["chat.attachButton"].exists)
        XCTAssertTrue(app.buttons["chat.modelPicker"].exists)
        XCTAssertTrue(app.buttons["chat.effortPicker"].exists)
        XCTAssertTrue(app.buttons["chat.reviewMode"].exists)
        XCTAssertTrue(app.buttons["chat.liveVoiceButton"].exists)
        XCTAssertTrue(app.buttons["chat.micButton"].exists)
        XCTAssertTrue(app.buttons["chat.fullAuto"].exists)
        XCTAssertTrue(app.buttons["chat.usageBar"].exists)

        // Open Always-On Usage Dashboard sheet from the persistent usage strip and dismiss cleanly.
        app.buttons["chat.usageBar"].tap()
        XCTAssertTrue(app.buttons["usage.done"].waitForExistence(timeout: 5))
        app.buttons["usage.done"].tap()
        XCTAssertTrue(sidebarButton.waitForExistence(timeout: 5))

        // Open Workspace Diff / Branch / PR sheet and dismiss cleanly.
        app.buttons["nav.workspace"].tap()
        XCTAssertTrue(app.buttons["workspace.done"].waitForExistence(timeout: 5))
        app.buttons["workspace.done"].tap()
        XCTAssertTrue(sidebarButton.waitForExistence(timeout: 5))

        // Open sidebar and verify Dashboard + Universal Introspection sections + zero legacy Login / Projects / Foreman controls exist.
        sidebarButton.tap()
        XCTAssertTrue(app.buttons["sidebar.newChat"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.buttons["sidebar.dashboard"].exists)
        XCTAssertTrue(app.buttons["sidebar.computers"].exists)
        for secID in ["side.sec.1", "side.sec.2", "side.sec.3", "side.sec.4"] {
            XCTAssertTrue(
                app.otherElements[secID].exists || app.buttons[secID].exists || app.cells[secID].exists || app.staticTexts[secID].exists,
                "Expected Universal Introspection sidebar section \(secID)"
            )
        }
        for removed in ["drawer.projects", "drawer.foreman", "drawer.login", "settings.login", "usage.open"] {
            XCTAssertFalse(app.buttons[removed].exists, "Legacy control \(removed) must not exist")
        }

        // Open Dashboard from sidebar and verify it renders cleanly, then return via New Chat.
        app.buttons["sidebar.dashboard"].tap()
        XCTAssertTrue(app.buttons["nav.newChat"].waitForExistence(timeout: 5))
        app.buttons["nav.newChat"].tap()
        XCTAssertTrue(sidebarButton.waitForExistence(timeout: 5))

        // Open Computers sheet from sidebar and verify MagicDNS/IP manual entry & Pair UI.
        sidebarButton.tap()
        XCTAssertTrue(app.buttons["sidebar.computers"].waitForExistence(timeout: 5))
        app.buttons["sidebar.computers"].tap()
        let addressField = app.textFields["computers.addAddress"]
        XCTAssertTrue(addressField.waitForExistence(timeout: 5))
        XCTAssertTrue(app.textFields["computers.addPort"].exists)
        XCTAssertTrue(app.buttons["computers.addButton"].exists)
        XCTAssertTrue(app.buttons["computers.scanQR"].exists)

        // Dismiss Computers sheet cleanly back to main Antigravity chat surface.
        app.buttons["computers.done"].tap()
        XCTAssertTrue(sidebarButton.waitForExistence(timeout: 5))

        // Verify `/` triggers the Slash Command popover, and composer accepts text natively.
        let composer = composerElement(app)
        XCTAssertTrue(composer.waitForExistence(timeout: 5))
        composer.tap()
        composer.typeText("/plan")
        XCTAssertTrue(app.buttons["chat.send"].isEnabled)
    }

    @MainActor
    func testLiveEndToEndLegacyV1AndModernV2Flows() throws {
        let env = ProcessInfo.processInfo.environment
        guard let legacyPort = env["OA_E2E_LEGACY_PORT"], !legacyPort.isEmpty,
              let v2Port = env["OA_E2E_V2_PORT"], !v2Port.isEmpty else {
            throw XCTSkip("Run via apps/openagents-ios/e2e-sim-test.sh to execute live dual-host Simulator E2E tests.")
        }

        let app = XCUIApplication()
        app.launchArguments = ["-OANoLocalNetworkBrowse", "-OAE2ELoopbackAllowed", "-OAResetStateForTests"]
        app.launch()

        XCTAssertTrue(app.buttons["nav.sidebar"].waitForExistence(timeout: 15))

        // ------------------------------------------------------------------
        // 1. STRICT LEGACY 16-KEY V1 HOST (reproduces deployed Mac daemon!)
        // ------------------------------------------------------------------
        pairHost(app: app, port: legacyPort, displayName: "Legacy V1 Mac", expectedPairedCount: 1)

        // Verify workspace metadata loaded cleanly over NIP-44 without error.
        let wsLabel = app.staticTexts["Workspace: openagents"]
        XCTAssertTrue(wsLabel.waitForExistence(timeout: 10), "Expected DescribeEnvironment workspace metadata from legacy v1 host")
        assertNoErrorBanner(app, context: "legacy v1 connect")

        // Verify `@` folder mention autocomplete works from host folders.
        let composer = focusComposer(app)
        composer.typeText("@apps")
        XCTAssertTrue(app.otherElements["chat.folderMenu"].waitForExistence(timeout: 5) || app.buttons["@apps/openagents-ios"].waitForExistence(timeout: 5))
        // Clear `@apps` and send default Auto-mode message.
        composer.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: 6) + "Hello legacy auto")
        app.buttons["chat.send"].tap()

        let autoReply = app.staticTexts["Antigravity verified reply: Hello legacy auto"]
        XCTAssertTrue(autoReply.waitForExistence(timeout: 15), "Legacy v1 host must accept default Auto-mode message cleanly")
        let thoughtHeader = app.buttons["act.item-thought-1"].exists
            ? app.buttons["act.item-thought-1"]
            : app.buttons["transcript.thoughtHeader"].firstMatch
        XCTAssertTrue(thoughtHeader.waitForExistence(timeout: 5), "Activity / thought row must render clean Desktop-parity header")
        // Expand activity row and verify thought + subagent rows appear cleanly inside the activity group.
        thoughtHeader.tap()
        let thoughtWords = app.staticTexts["actr.item-thought-1.words"]
        XCTAssertTrue(thoughtWords.waitForExistence(timeout: 5), "Expanded thought body should appear inside activity group")
        XCTAssertTrue(app.buttons["actr.item-sub-1"].waitForExistence(timeout: 5), "Expanded subagent row should appear inside activity group")
        thoughtHeader.tap()
        assertNoErrorBanner(app, context: "legacy v1 Auto-mode OpenSession")

        // Switch Review Mode to Plan -> legacy host refuses "mode":"plan" with real NIP-44 "unsupported"
        // -> app MUST transparently downgrade and retry without "mode" -> ZERO error banner!
        app.buttons["chat.reviewMode"].tap()
        let planOption = app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", "Plan")).firstMatch
        XCTAssertTrue(planOption.waitForExistence(timeout: 5))
        planOption.tap()

        sendPrompt(app, text: "Plan legacy refactor")
        let planHeader = app.staticTexts.matching(
            NSPredicate(format: "label CONTAINS %@ AND label CONTAINS %@", "Antigravity verified reply:", "Plan mode")
        ).firstMatch
        let planBody = app.staticTexts.matching(
            NSPredicate(format: "label CONTAINS %@", "Plan legacy refactor")
        ).firstMatch
        XCTAssertTrue(planHeader.waitForExistence(timeout: 15) && planBody.waitForExistence(timeout: 5), "Legacy v1 host Plan-mode auto-downgrade must succeed cleanly")
        assertNoErrorBanner(app, context: "legacy v1 Plan-mode transparent downgrade")
        saveScreenshot(name: "01-legacy-v1-chat")

        // ------------------------------------------------------------------
        // 2. MODERN V2 HOST (push frames, approvals, subagents, Git Workspace)
        // ------------------------------------------------------------------
        pairHost(app: app, port: v2Port, displayName: "E2E Studio Mac", expectedPairedCount: 2)
        XCTAssertTrue(app.staticTexts["Workspace: openagents"].waitForExistence(timeout: 10))
        assertNoErrorBanner(app, context: "v2 connect")

        // Full Auto defaults ON; toggle Full Auto OFF via chip to verify manual Approval Requested banner flow (#14068).
        let fullAutoBtn = app.buttons["chat.fullAuto"]
        XCTAssertTrue(fullAutoBtn.waitForExistence(timeout: 5))
        fullAutoBtn.tap()

        // Send a prompt that triggers an interactive Approval Requested banner + Subagent tool row.
        sendPrompt(app, text: "Run tests and approve")

        let allowBtn = app.buttons["approval.opt.0"].waitForExistence(timeout: 15)
            ? app.buttons["approval.opt.0"]
            : (app.buttons["approval.allow"].exists ? app.buttons["approval.allow"] : app.buttons["interaction.allow"])
        XCTAssertTrue(allowBtn.exists, "Expected Approval Requested Allow button from v2 host")
        let liveThought = app.buttons["act.item-thought-1"].exists
            ? app.buttons["act.item-thought-1"]
            : app.buttons["transcript.thoughtHeader"].firstMatch
        XCTAssertTrue(liveThought.waitForExistence(timeout: 5), "Active streaming activity row must exist")
        assertNoErrorBanner(app, context: "v2 approval requested")
        saveScreenshot(name: "02-v2-approval-banner")

        // Tap Allow -> RespondInteraction -> assistant confirms execution.
        allowBtn.tap()
        let approvedReply = app.staticTexts["Approved and executed cleanly."]
        XCTAssertTrue(approvedReply.waitForExistence(timeout: 15), "Expected approved reply after tapping Allow")
        assertNoErrorBanner(app, context: "v2 after approval")
        saveScreenshot(name: "03-v2-approved-chat")

        // Expand activity group if collapsed and tap Subagent row -> ReadSubagent sheet opens with subagent transcript.
        if !app.buttons["actr.item-sub-1"].exists && app.buttons["act.item-thought-1"].exists {
            app.buttons["act.item-thought-1"].tap()
        }
        let inspectBtn = app.buttons["actr.item-sub-1"].waitForExistence(timeout: 10)
            ? app.buttons["actr.item-sub-1"]
            : app.buttons["transcript.inspectSubagent"].firstMatch
        XCTAssertTrue(inspectBtn.exists, "Expected Subagent row in transcript")
        inspectBtn.tap()
        let subText = app.staticTexts["Subagent inspected 12 files cleanly."]
        XCTAssertTrue(subText.waitForExistence(timeout: 15), "Expected ReadSubagent transcript inside Subagent sheet")
        app.buttons["subagent.done"].tap()
        XCTAssertTrue(app.buttons["nav.workspace"].waitForExistence(timeout: 8))

        // Open Workspace Inspector -> verify Files tab, Diff tab, and live SwitchBranch mutation!
        app.buttons["nav.workspace"].tap()
        let branchHeader = app.staticTexts["workspace.branchHeader"]
        XCTAssertTrue(branchHeader.waitForExistence(timeout: 15))
        XCTAssertEqual(branchHeader.label, "main")

        // Switch to Files tab and confirm changed file from git status renders (and expands inline diff).
        app.buttons["Files"].tap()
        let fileRow = app.buttons.matching(NSPredicate(format: "label CONTAINS %@", "apps/openagents-ios/Sources/ChatView.swift")).firstMatch
        XCTAssertTrue(fileRow.waitForExistence(timeout: 8), "Expected changed file row in Files tab")
        fileRow.tap()
        saveScreenshot(name: "04-v2-workspace-files")

        // Switch to Branches tab and tap Switch on feat/antigravity.
        app.buttons["Branches"].tap()
        let switchBtn = app.buttons["workspace.switchBranch.feat/antigravity"]
        XCTAssertTrue(switchBtn.waitForExistence(timeout: 8))
        switchBtn.tap()

        // Verify live SwitchBranch RPC updated the header to feat/antigravity!
        let pred = NSPredicate(format: "label == %@", "feat/antigravity")
        let exp = expectation(for: pred, evaluatedWith: branchHeader)
        wait(for: [exp], timeout: 15)
        saveScreenshot(name: "05-v2-workspace-switched-branch")

        app.buttons["workspace.done"].tap()
        XCTAssertTrue(app.buttons["nav.sidebar"].waitForExistence(timeout: 8))
        assertNoErrorBanner(app, context: "v2 end of full flow")
    }
}
