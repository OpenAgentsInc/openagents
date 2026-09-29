import XCTest

// Taps through the spec's flows the way a playtester would, and counts the
// taps from app open to a run starting (spec CHK-06: first time 3,
// returning 2). Each test starts its flow over with the Screen index's
// FLOW-nn.start entry, passed as the --screen launch argument.

final class FlowTests: XCTestCase {
    private var app: XCUIApplication!
    private var taps = 0

    override func setUp() {
        continueAfterFailure = false
        app = XCUIApplication()
    }

    private func launch(_ screen: String) {
        app.launchArguments = ["--screen", screen]
        app.launch()
        taps = 0
    }

    /// The button labeled `label` that a finger can reach: the newest one
    /// on top (a sheet repeats its card's button, and the card's sits under it).
    private func button(_ label: String, timeout: TimeInterval = 8) -> XCUIElement {
        let query = app.buttons.matching(NSPredicate(format: "label CONTAINS[c] %@", label))
        XCTAssertTrue(query.firstMatch.waitForExistence(timeout: timeout), "No \"\(label)\" button")
        for attempt in 0..<2 {
            for i in stride(from: query.count - 1, through: 0, by: -1) where query.element(boundBy: i).isHittable {
                return query.element(boundBy: i)
            }
            if attempt == 0 { app.swipeUp() }
        }
        return query.element(boundBy: query.count - 1)
    }

    private func tap(_ label: String, timeout: TimeInterval = 8) {
        button(label, timeout: timeout).tap()
        taps += 1
    }

    private func waitForText(_ text: String, timeout: TimeInterval = 8) {
        let t = app.staticTexts.matching(NSPredicate(format: "label CONTAINS[c] %@", text)).firstMatch
        XCTAssertTrue(t.waitForExistence(timeout: timeout), "No text \"\(text)\"")
    }

    /// FLOW-01: CHOOSE CODER, LET'S GO (after the cinematic plays through),
    /// START THE TEST: exactly 3 taps. Then the result, Add to the Gym,
    /// Level up, and the main menu. A relaunch mid-run resumes the run.
    func testFlow01FirstTimePlaytester() {
        launch("FLOW-01.start")
        tap("Choose Coder")
        tap("Let's go", timeout: 60)
        waitForText("Let's see if a tool makes Coder better")
        tap("Start the test")
        XCTAssertEqual(taps, 3, "FLOW-01 must start the first run in 3 taps")
        waitForText("Testing Project map")

        // FLOW-01 rule 1: relaunch resumes the first-run chat at the run.
        app.terminate()
        app.launchArguments = []
        app.launch()
        waitForText("STEP 3 OF 3", timeout: 20)

        tap("Add to the Gym", timeout: 30)
        waitForText("Everyone will see")
        tap("Add to the Gym")
        tap("Nice", timeout: 20)
        XCTAssertTrue(button("Chat with OpenAgents").exists, "FLOW-01 ends at the main menu")
    }

    /// FLOW-02: from the menu, a starter chip and START THE TEST: 2 taps.
    func testFlow02ReturningPlaytester() {
        launch("FLOW-02.start")
        tap("Test a tool")
        tap("Start the test")
        XCTAssertEqual(taps, 2, "FLOW-02 must start a run in 2 taps")
        waitForText("Testing Project map")
        tap("Add to the Gym", timeout: 30)
        tap("Add to the Gym")
        waitForText("Added to the Gym")
    }

    /// FLOW-07: the interview, the draft, a first try, the full run, Add to the Gym.
    func testFlow07MakeAToolInChat() {
        launch("FLOW-07.start")
        tap("One line, past tense. It names the fix and links the change.")
        tap("Looks good")                       // the offer chip
        waitForText("YOUR TEST SET")
        tap("Looks good")                       // CARD-02: the tests
        waitForText("Now, how each test is checked")
        tap("Looks good")                       // CARD-02: the checks
        tap("Try it once")
        waitForText("Trying Changelog helper once")
        tap("Run the full test set", timeout: 20)
        waitForText("Testing Changelog helper")
        tap("Add to the Gym", timeout: 30)
        waitForText("your tool and your 5 tests")
        tap("Add to the Gym")
        waitForText("Added to the Gym")
    }

    /// FLOW-08: a check is waiting; CHAT WITH OPENAGENTS opens CARD-06 and
    /// RUN THE CHECK starts it: 2 taps. Then YOU CONFIRMED IT.
    func testFlow08CheckAResult() {
        launch("FLOW-08.start")
        tap("Chat with OpenAgents")
        tap("Run the check")
        XCTAssertEqual(taps, 2, "A waiting check starts in 2 taps")
        waitForText("Checking Trainer 2PX's result")
        waitForText("YOU CONFIRMED IT", timeout: 30)
    }
}
