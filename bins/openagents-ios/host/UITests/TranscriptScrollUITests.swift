// The transcript scrolls under a finger. Build 28 and earlier drew chats
// that could not be scrolled: the transcript answered its gesture delegate
// for its own pan recognizer as it did for its selection tap, so the pan
// never got a touch. This opens Rust Native's conversation fixture, long
// enough to scroll, and drags it.
import XCTest

final class TranscriptScrollUITests: XCTestCase {
    func testALongTranscriptScrollsUnderAFinger() throws {
        continueAfterFailure = false
        let app = XCUIApplication()
        app.launchArguments = ["--rust-native-fixture", "--rust-native-fixture-rows", "200",
                               "--rust-native-transcript-pull"]
        app.launch()
        let transcript = app.scrollViews["transcript"]
        XCTAssertTrue(transcript.waitForExistence(timeout: 30))
        // It opens at the newest row; a drag down reads back up it, and the
        // way back to the bottom appears.
        let bottom = app.buttons["transcript-scroll-to-bottom"]
        XCTAssertFalse(bottom.isHittable)
        transcript.swipeDown()
        XCTAssertTrue(bottom.waitForExistence(timeout: 5))
        let shown = expectation(for: NSPredicate(format: "isHittable == true"), evaluatedWith: bottom)
        wait(for: [shown], timeout: 5)
        bottom.tap()
        let gone = expectation(for: NSPredicate(format: "isHittable == false"), evaluatedWith: bottom)
        wait(for: [gone], timeout: 5)
    }
}
