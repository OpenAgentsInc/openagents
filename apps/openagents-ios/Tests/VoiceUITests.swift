import XCTest

/// Voice in the Grid against a real voice host (metareal/voice-host), with a
/// bundled recording streamed through the same uplink path as the microphone
/// (`-OAVoiceFixture`). Run by apps/openagents-ios/tools/voice-sim-test.sh,
/// which starts the host and passes these through TEST_RUNNER_ variables:
///   OA_VOICE_HOST     ws://127.0.0.1:PORT (default ws://127.0.0.1:8787)
///   OA_VOICE_FIXTURE  a bundled WAV name (default hello-foreman)
///   OA_VOICE_EXPECT   words the transcript must contain (live Google runs)
///   OA_VOICE_AGENT    echo (default: the reply repeats the transcript) or
///                     gemini (any other non-empty reply)
final class VoiceUITests: XCTestCase {
    private func shot(_ app: XCUIApplication, _ name: String) {
        let a = XCTAttachment(screenshot: app.screenshot()); a.name = name; a.lifetime = .keepAlways; add(a)
    }
    private func wait(_ timeout: TimeInterval, _ test: @escaping () -> Bool) -> Bool {
        XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in test() }, object: nil)], timeout: timeout) == .completed
    }

    func testATurnWithTheFixtureIsHeardAndAnswered() throws {
        let env = ProcessInfo.processInfo.environment
        let host = env["OA_VOICE_HOST"] ?? "ws://127.0.0.1:8787"
        let fixture = env["OA_VOICE_FIXTURE"] ?? "hello-foreman"
        let echo = (env["OA_VOICE_AGENT"] ?? "echo") == "echo"
        let expected = (env["OA_VOICE_EXPECT"] ?? "").lowercased().split(separator: " ").map(String.init)

        let app = XCUIApplication()
        app.launchArguments += ["-OAVoiceHost", host, "-OAVoiceFixture", fixture, "-OAVoiceObserve", "YES"]
        app.launch()
        let start = app.buttons["home.start"]
        XCTAssertTrue(start.waitForExistence(timeout: 20))
        // No voice over the overview: it is a Grid control.
        XCTAssertFalse(app.buttons["voice.toggle"].exists)
        start.tap()
        let toggle = app.buttons["voice.toggle"]
        XCTAssertTrue(toggle.waitForExistence(timeout: 20))
        XCTAssertEqual(toggle.value as? String, "off")
        XCTAssertLessThan(toggle.frame.minX, app.frame.width / 2, "the voice toggle sits top left")
        XCTAssertLessThan(toggle.frame.minY, 120)

        toggle.tap()
        // The first time, the app says where speech goes.
        let notice = app.alerts["Voice"]
        if notice.waitForExistence(timeout: 3) {
            XCTAssertTrue(notice.staticTexts.containing(NSPredicate(format: "label CONTAINS 'Google'")).count > 0)
            shot(app, "Voice notice")
            notice.buttons["Turn on"].tap()
        }

        var phases = Set<String>(), kinds = Set<String>()
        let caption = app.staticTexts["voice.caption"]
        let level = app.otherElements["voice.level"]
        XCTAssertTrue(level.waitForExistence(timeout: 5))
        let watch = {
            if let p = toggle.value as? String { phases.insert(p) }
            if caption.exists, let v = caption.value as? String, let k = v.split(separator: ";").first { kinds.insert(String(k)) }
        }
        // The host answers, the recording plays in, and voice.coax opens a turn.
        // "level L peak P turns T replies R"
        let count = { (key: String) -> Int in
            (level.value as? String)?.components(separatedBy: key + " ").last?.split(separator: " ").first.flatMap { Int($0) } ?? 0
        }
        let turns = { count("turns") }
        XCTAssertTrue(wait(30) { watch(); return turns() >= 1 }, "phases: \(phases), level: \(level.value ?? "")")
        shot(app, "Hearing")

        // The final transcript arrives (kept as "heard" while the reply plays).
        var heard = ""
        XCTAssertTrue(wait(45) {
            watch()
            guard caption.exists, let v = caption.value as? String, let r = v.range(of: "; heard: ") else { return false }
            heard = String(v[r.upperBound...]); return !heard.isEmpty
        }, "no final transcript; phases \(phases), caption: \(caption.exists ? caption.label : "-")")
        NSLog("VoiceUITests heard: %@", heard)
        for word in expected {
            XCTAssertTrue(heard.lowercased().contains(word), "\"\(word)\" not in the transcript \"\(heard)\"")
        }

        // Then the agent's reply, spoken and shown as it plays, to the last word.
        var reply = ""
        XCTAssertTrue(wait(45) {
            watch()
            guard caption.exists, (caption.value as? String)?.hasPrefix("reply") == true else { return false }
            reply = caption.label
            // SPEECH_END arrived and playback finished: the whole reply was heard.
            return !reply.isEmpty && count("replies") >= 1 && toggle.value as? String == "listening"
        }, "no reply; phases \(phases), caption: \(caption.exists ? caption.label : "-")")
        NSLog("VoiceUITests reply: %@", reply)
        XCTAssertTrue(kinds.contains("final") || kinds.contains("partial") || kinds.contains("reply"), "kinds: \(kinds)")
        if echo {
            XCTAssertEqual(reply, "You said: " + heard)
        } else {
            XCTAssertFalse(reply.hasPrefix("You said"), "the agent echoed: \(reply)")
            XCTAssertNotEqual(reply, heard)
        }
        shot(app, "Reply")

        // The level indicator followed the recording.
        let peak = count("peak")
        XCTAssertGreaterThan(peak, 20, "level: \(level.value ?? "")")

        toggle.tap()
        XCTAssertTrue(wait(5) { toggle.value as? String == "off" })
        XCTAssertTrue(!caption.exists || caption.label.isEmpty, "caption after off: \(caption.label)")
        app.terminate()
    }
}
