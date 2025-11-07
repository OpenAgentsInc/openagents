//
//  OpenAgentsUITestsLaunchTests.swift
//  OpenAgentsUITests
//
//  Created by Christopher David on 11/3/25.
//

import XCTest

final class OpenAgentsUITestsLaunchTests: XCTestCase {

    override class var runsForEachTargetApplicationUIConfiguration: Bool {
        true
    }

    override func setUpWithError() throws {
        // Skip all UI tests unless explicitly enabled via environment variable
        guard ProcessInfo.processInfo.environment["ENABLE_UI_TESTS"] == "1" else {
            throw XCTSkip("UI tests disabled by default. Set ENABLE_UI_TESTS=1 to run.")
        }

        continueAfterFailure = false
    }

    @MainActor
    func testLaunch() throws {
        let app = XCUIApplication()
        app.launch()

        // Insert steps here to perform after app launch but before taking a screenshot,
        // such as logging into a test account or navigating somewhere in the app

        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = "Launch Screen"
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
