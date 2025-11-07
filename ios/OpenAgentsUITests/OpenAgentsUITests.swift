//
//  OpenAgentsUITests.swift
//  OpenAgentsUITests
//
//  Created by Christopher David on 11/3/25.
//

import XCTest

final class OpenAgentsUITests: XCTestCase {

    override func setUpWithError() throws {
        // Skip all UI tests unless explicitly enabled via environment variable
        guard ProcessInfo.processInfo.environment["ENABLE_UI_TESTS"] == "1" else {
            throw XCTSkip("UI tests disabled by default. Set ENABLE_UI_TESTS=1 to run.")
        }

        // Put setup code here. This method is called before the invocation of each test method in the class.

        // In UI tests it is usually best to stop immediately when a failure occurs.
        continueAfterFailure = false

        // In UI tests it's important to set the initial state - such as interface orientation - required for your tests before they run. The setUp method is a good place to do this.
    }

    override func tearDownWithError() throws {
        // Put teardown code here. This method is called after the invocation of each test method in the class.
    }

    @MainActor
    func testExample() throws {
        // UI tests must launch the application that they test.
        let app = XCUIApplication()
        app.launch()

        // Use XCTAssert and related functions to verify your tests produce the correct results.
    }

    @MainActor
    func testLaunchPerformance() throws {
        // This measures how long it takes to launch your application.
        measure(metrics: [XCTApplicationLaunchMetric()]) {
            XCUIApplication().launch()
        }
    }
}
