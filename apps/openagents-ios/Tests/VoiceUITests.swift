import XCTest

/// Voice in the Grid against a real voice host (verse/voice-host), with a
/// bundled recording streamed through the same uplink path as the microphone
/// (`-OAVoiceFixture`). Run by apps/openagents-ios/tools/voice-sim-test.sh,
/// which starts the host and passes these through TEST_RUNNER_ variables:
///   OA_VOICE_HOST     ws://127.0.0.1:PORT (default ws://127.0.0.1:8787)
///   OA_VOICE_FIXTURE  a bundled WAV name (default hello-foreman)
///   OA_VOICE_EXPECT   words the transcript must contain (live Google runs)
///   OA_VOICE_AGENT    echo (default: the reply repeats the transcript) or
///                     gemini (any other non-empty reply)
///   OA_VOICE_PLAYBACK_DELAY  seconds of simulated phone output latency
final class VoiceUITests: XCTestCase {
    private func shot(_ app: XCUIApplication, _ name: String) {
        let a = XCTAttachment(screenshot: app.screenshot()); a.name = name; a.lifetime = .keepAlways; add(a)
    }
    /// Launch arguments for a voice run against the test host.
    private func voiceArguments(_ env: [String: String], host: String, fixture: String) -> [String] {
        ["-OAVoiceHost", host, "-OAVoiceFixture", fixture, "-OAVoiceObserve", "YES",
         "-OAVoicePlaybackDelay", env["OA_VOICE_PLAYBACK_DELAY"] ?? "0.25"]
    }
    /// The reply as a listener hears it, from the app's own player measurements.
    private func checkPlayback(_ count: (String) -> Int, _ level: XCUIElement) {
        let first = count("first"), play = count("play"), audio = count("audio"), dry = count("dry"), drops = count("drops")
        NSLog("VoiceUITests playback: first %d ms, play %d ms, audio %d ms, dry %d, caption drops %d", first, play, audio, dry, drops)
        XCTAssertGreaterThan(audio, 200, "no reply audio measured: \(level.value ?? "")")
        XCTAssertLessThan(first, 1500, "the reply took \(first) ms to start playing")
        // About its own length: a stall or audio played twice runs long.
        XCTAssertLessThan(Double(play), Double(audio) * 1.25 + 600, "\(audio) ms of audio took \(play) ms to play")
        XCTAssertGreaterThan(Double(play), Double(audio) * 0.8, "\(audio) ms of audio played in \(play) ms")
        XCTAssertEqual(dry, 0, "the player ran dry \(dry) time(s) mid-reply (choppy)")
        XCTAssertEqual(drops, 0, "the reply caption lost words \(drops) time(s)")
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

        app.launchArguments += ["-OAGridSeen", "NO", "-OANativeHome", "YES"]  // every test starts from the first-launch overview
        app.launchArguments += voiceArguments(env, host: host, fixture: fixture)
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

        checkPlayback(count, level)

        // The level indicator followed the recording.
        let peak = count("peak")
        XCTAssertGreaterThan(peak, 20, "level: \(level.value ?? "")")

        toggle.tap()
        XCTAssertTrue(wait(5) { toggle.value as? String == "off" })
        XCTAssertTrue(!caption.exists || caption.label.isEmpty, "caption after off: \(caption.label)")
        app.terminate()
    }

    /// Talking over the reply: a second recording plays once the reply has
    /// played 300 ms. voice.coax hears it, the phone cuts playback and reports
    /// how much was heard, the new turn is recognized and answered.
    func testTalkingOverTheReplyStopsItAndIsHeard() throws {
        let env = ProcessInfo.processInfo.environment
        let host = env["OA_VOICE_HOST"] ?? "ws://127.0.0.1:8787"
        let fixture = env["OA_VOICE_FIXTURE"] ?? "hello-foreman"
        let app = XCUIApplication()
        app.launchArguments += ["-OAGridSeen", "NO", "-OANativeHome", "YES"]  // every test starts from the first-launch overview
        app.launchArguments += voiceArguments(env, host: host, fixture: fixture) + ["-OAVoiceBarge", "wait-stop"]
        app.launch()
        let start = app.buttons["home.start"]
        XCTAssertTrue(start.waitForExistence(timeout: 20))
        start.tap()
        let toggle = app.buttons["voice.toggle"]
        XCTAssertTrue(toggle.waitForExistence(timeout: 20))
        toggle.tap()
        let notice = app.alerts["Voice"]
        if notice.waitForExistence(timeout: 3) { notice.buttons["Turn on"].tap() }
        let level = app.otherElements["voice.level"]
        let caption = app.staticTexts["voice.caption"]
        XCTAssertTrue(level.waitForExistence(timeout: 5))
        let count = { (key: String) -> Int in
            (level.value as? String)?.components(separatedBy: key + " ").last?.split(separator: " ").first.flatMap { Int($0) } ?? 0
        }
        var firstHeard = ""
        XCTAssertTrue(wait(60) {
            if firstHeard.isEmpty, caption.exists, let v = caption.value as? String, let r = v.range(of: "; heard: ") { firstHeard = String(v[r.upperBound...]) }
            return count("barges") >= 1
        }, "never barged in; level: \(level.value ?? "")")
        shot(app, "Barge-in")
        XCTAssertTrue(wait(45) { count("turns") >= 2 }, "no second turn; level: \(level.value ?? "")")
        var heard = ""
        XCTAssertTrue(wait(60) {
            guard caption.exists, let v = caption.value as? String, let r = v.range(of: "; heard: ") else { return false }
            heard = String(v[r.upperBound...])
            return !heard.isEmpty && heard != firstHeard && v.hasPrefix("reply") && toggle.value as? String == "listening"
        }, "the second turn was not answered; caption: \(caption.exists ? (caption.value as? String ?? "") : "-")")
        NSLog("VoiceUITests barge heard: %@ (first: %@) level: %@", heard, firstHeard, (level.value as? String) ?? "")
        XCTAssertGreaterThanOrEqual(count("decisions"), 1, "no frame decision was applied; level: \(level.value ?? "")")
        shot(app, "Answered after barge-in")
        app.terminate()
    }
}
