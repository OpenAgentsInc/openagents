import XCTest

/// Plays the game (its bot) and turns the device while it runs: the web
/// view must fill the screen in every orientation, with no card, inset, or
/// wallpaper. With ICECREAM_SHOTS set to a folder (pass it to xcodebuild as
/// TEST_RUNNER_ICECREAM_SHOTS), it saves a whole-screen PNG per orientation
/// there for the store and for review (ICECREAM_SEED picks the bot's run).
final class RotationUITests: XCTestCase {
    func testFillsTheScreenWhileTurning() throws {
        let app = XCUIApplication()
        let environment = ProcessInfo.processInfo.environment
        let seed = environment["ICECREAM_SEED"].flatMap { $0.isEmpty ? nil : $0 } ?? "7"
        app.launchArguments = ["-IceCreamFragment", "bot&seed=\(seed)"]
        let device = XCUIDevice.shared
        device.orientation = .portrait
        app.launch()
        let web = app.webViews.firstMatch
        XCTAssertTrue(web.waitForExistence(timeout: 20))
        sleep(10)
        let idiom = UIDevice.current.userInterfaceIdiom == .pad ? "ipad" : "iphone"
        for (orientation, name) in [
            (UIDeviceOrientation.portrait, "portrait"),
            (.landscapeLeft, "landscape"),
            (.portrait, "portrait-again"),
            (.landscapeRight, "landscape-right"),
        ] {
            device.orientation = orientation
            sleep(6)
            let screen = XCUIScreen.main.screenshot()
            let frame = web.frame
            let window = app.windows.firstMatch.frame
            XCTAssertEqual(frame.width, window.width, accuracy: 1, "\(name): web view width")
            XCTAssertEqual(frame.height, window.height, accuracy: 1, "\(name): web view height")
            XCTAssertEqual(window.width * window.height, screen.image.size.width * screen.image.size.height,
                           accuracy: 4, "\(name): the window fills the screen")
            let attachment = XCTAttachment(screenshot: screen)
            attachment.name = "\(idiom)-\(name)"
            attachment.lifetime = .keepAlways
            add(attachment)
            if let folder = environment["ICECREAM_SHOTS"], !folder.isEmpty {
                let url = URL(fileURLWithPath: folder).appendingPathComponent("\(idiom)-\(name)-\(seed).png")
                try screen.pngRepresentation.write(to: url)
            }
        }
        device.orientation = .portrait
        sleep(4)
        // The pause menu shows the app's version (the canvas's data-version).
        let menu = app.webViews.buttons.matching(NSPredicate(format: "label CONTAINS 'Menu'")).firstMatch
        if menu.waitForExistence(timeout: 5) {
            menu.tap()
            sleep(2)
            let shot = XCUIScreen.main.screenshot()
            if let folder = environment["ICECREAM_SHOTS"], !folder.isEmpty {
                try shot.pngRepresentation.write(
                    to: URL(fileURLWithPath: folder).appendingPathComponent("\(idiom)-menu-\(seed).png"))
            }
        }
    }
}
