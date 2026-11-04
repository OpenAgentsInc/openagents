// Firm reason: 6 (AppKit and WKWebView platform effects). World/editor logic is
// Coax; this host owns the window, bundled child processes, sockets and files.
import AppKit
import WebKit
import AVFoundation
import Darwin

enum ShellError: LocalizedError {
    case failed(String)
    var errorDescription: String? { if case .failed(let s) = self { return s }; return nil }
}

// Keep both reservations until we have chosen distinct ports. The children bind
// immediately after release; startup checks surface a bind race as a failure.
func reservePort() throws -> (Int32, Int) {
    let fd = socket(AF_INET, SOCK_STREAM, 0)
    guard fd >= 0 else { throw ShellError.failed("Cannot reserve a local port.") }
    var addr = sockaddr_in()
    addr.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
    addr.sin_family = sa_family_t(AF_INET)
    addr.sin_addr.s_addr = inet_addr("127.0.0.1")
    let result = withUnsafePointer(to: &addr) {
        $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { bind(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size)) }
    }
    var length = socklen_t(MemoryLayout<sockaddr_in>.size)
    let read = withUnsafeMutablePointer(to: &addr) {
        $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { getsockname(fd, $0, &length) }
    }
    guard result == 0, read == 0 else { close(fd); throw ShellError.failed("Cannot bind a local port.") }
    return (fd, Int(UInt16(bigEndian: addr.sin_port)))
}

@MainActor
final class ForgeWindow: NSWindow {
    var coderKey: ((NSEvent) -> Bool)?
    override func sendEvent(_ event: NSEvent) {
        if coderKey?(event) == true { return }
        super.sendEvent(event)
    }
}

@MainActor
final class ForgeApp: NSObject, NSApplicationDelegate, NSWindowDelegate, WKNavigationDelegate, WKUIDelegate, WKScriptMessageHandler {
    let fm = FileManager.default
    var window: ForgeWindow!
    var coderHeldKeys: Set<UInt16> = []
    var web: WKWebView!
    var notice: NSTextField!
    var room: Process?
    var page: Process?
    var voiceHost: Process?
    var voiceStartup: Task<Void, Error>?
    var voiceReservation: (Int32, Int)?
    var voicePort = 0
    var logs: [FileHandle] = []
    var startup: Task<Void, Never>?
    var framePump: Task<Void, Never>?
    var inventoryTask: Task<Void, Never>?
    var setupPane: ForgeSetupPane!
    var chatPane: ForgeChatPane!
    var sceneReady = false
    var startupPhase: Int64 = 0
    var retryCheckStarted = false
    var retryCheckPassed = false
    var hiddenSince: Double?
    var longestHiddenSeconds = 0.0
    var dataURL: URL!
    var pageURL: URL!
    var lockFD: Int32 = -1
    var stopping = false
    let startedAt = Date()
    var firstFrameSeconds = 0.0
    let setupSmoke = CommandLine.arguments.contains("--setup-smoke")
    let voiceSmoke = CommandLine.arguments.contains("--voice-smoke")
    let chatSmoke = CommandLine.arguments.contains("--chat-smoke")
    let inputSmoke = CommandLine.arguments.contains("--input-smoke")
    var launchReport: URL? {
        guard let i = CommandLine.arguments.firstIndex(of: "--launch-check"), i + 1 < CommandLine.arguments.count else { return nil }
        return URL(fileURLWithPath: CommandLine.arguments[i + 1])
    }
    var smoke: Bool { inputSmoke || chatSmoke || setupSmoke || voiceSmoke || CommandLine.arguments.contains("--smoke-test") }
    var report: URL? {
        guard let i = CommandLine.arguments.firstIndex(of: inputSmoke ? "--input-smoke" : chatSmoke ? "--chat-smoke" : voiceSmoke ? "--voice-smoke" : setupSmoke ? "--setup-smoke" : "--smoke-test"), i + 1 < CommandLine.arguments.count else { return nil }
        return URL(fileURLWithPath: CommandLine.arguments[i + 1])
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(smoke ? .prohibited : .regular)
        makeWindow()
        startup = Task {
            if launchReport != nil && CommandLine.arguments.contains("--launch-check-background") {
                // Let AppKit finish its initial activation before minimizing;
                // an initial reopen event can otherwise undo a synchronous hide.
                try? await Task.sleep(for: .seconds(1))
                window.miniaturize(nil)
                Task {
                    try? await Task.sleep(for: .seconds(65))
                    guard !stopping else { return }
                    window.deminiaturize(nil); window.makeKeyAndOrderFront(nil); NSApp.activate(ignoringOtherApps: true)
                }
            }
            if chatSmoke { await chatSmokeCheck() } else { await start() }
        }
    }

    func makeWindow() {
        let menu = NSMenu()
        let appItem = NSMenuItem(); menu.addItem(appItem)
        let appMenu = NSMenu(); appItem.submenu = appMenu
        appMenu.addItem(withTitle: "Quit Aethyrforge", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        let editItem = NSMenuItem(); menu.addItem(editItem)
        let edit = NSMenu(title: "Edit"); editItem.submenu = edit
        for (name, selector, key) in [("Cut", "cut:", "x"), ("Copy", "copy:", "c"), ("Paste", "paste:", "v"), ("Select All", "selectAll:", "a")] {
            edit.addItem(withTitle: name, action: Selector(selector), keyEquivalent: key)
        }
        let viewItem = NSMenuItem(); menu.addItem(viewItem)
        let view = NSMenu(title: "View"); viewItem.submenu = view
        let reload = view.addItem(withTitle: "Reload Everglade", action: #selector(reloadWorld), keyEquivalent: "r"); reload.target = self
        let setup = view.addItem(withTitle: "Setup…", action: #selector(showSetup), keyEquivalent: ","); setup.target = self
        let chat = view.addItem(withTitle: "Message Coding Agent", action: #selector(focusChat), keyEquivalent: "l"); chat.target = self
        let folder = view.addItem(withTitle: "Open Local World Folder", action: #selector(openWorldFolder), keyEquivalent: "o"); folder.target = self
        folder.keyEquivalentModifierMask = [.command, .shift]
        NSApp.mainMenu = menu

        let config = WKWebViewConfiguration()
        config.userContentController.add(self, name: "forgeVoiceStatus")
        config.websiteDataStore = .nonPersistent()
        if launchReport != nil {
            config.userContentController.addUserScript(WKUserScript(source: "globalThis.__forgeFrameCount=0;const nativeRAF=requestAnimationFrame;globalThis.requestAnimationFrame=f=>nativeRAF(t=>{__forgeFrameCount++;f(t);});", injectionTime: .atDocumentStart, forMainFrameOnly: true))
        }
        config.mediaTypesRequiringUserActionForPlayback = []
        // Keep the audio graph processing microphone samples while muting output.
        config.userContentController.addUserScript(WKUserScript(source: "globalThis.coaxialAudioTap = c => {const g=c.createGain();g.gain.value=0;g.connect(c.destination);return g;};", injectionTime: .atDocumentStart, forMainFrameOnly: true))
        if smoke {
            // The native test pumps the callback: WebKit throttles both animation
            // callbacks and page timers in an unordered window.
            config.userContentController.addUserScript(WKUserScript(source: "globalThis.requestAnimationFrame = f => { globalThis.__forgeFrame = f; return 1; }; globalThis.cancelAnimationFrame = () => {globalThis.__forgeFrame = null;}; Object.defineProperty(document,'hidden',{get:()=>false}); Object.defineProperty(document,'visibilityState',{get:()=> 'visible'}); document.hasFocus=()=>true;", injectionTime: .atDocumentStart, forMainFrameOnly: true))
            config.userContentController.addUserScript(WKUserScript(source: "globalThis.__forgeRequests=[]; const originalFetch=fetch; globalThis.fetch=(...a)=>{const e={url:String(a[0]),method:a[1]?.method||'GET',status:'pending'};if(e.url.includes('world/everglade'))__forgeRequests.push(e);return originalFetch(...a).then(r=>{e.status=r.status;return r;},x=>{e.status=String(x);throw x;});};", injectionTime: .atDocumentStart, forMainFrameOnly: true))
        }
        web = WKWebView(frame: NSRect(x: 0, y: 0, width: 1280, height: 800), configuration: config)
        web.navigationDelegate = self; web.uiDelegate = self
        web.autoresizingMask = [.width, .height]
        window = ForgeWindow(contentRect: web.frame, styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
        window.coderKey = { [weak self] in self?.routeCoderKey($0) ?? false }
        window.delegate = self
        window.title = "Aethyrforge"
        window.subtitle = "Everglade · Local edits · Experimental"
        window.minSize = NSSize(width: 900, height: 740)
        let content = NSView(frame: web.frame); content.addSubview(web)
        window.contentView = content; window.center()
        notice = NSTextField(labelWithString: "Starting local Everglade…")
        notice.textColor = .white; notice.backgroundColor = .black; notice.drawsBackground = true
        notice.alignment = .center; notice.frame = NSRect(x: 0, y: 0, width: 1280, height: 28)
        notice.autoresizingMask = [.width, .maxYMargin]
        content.addSubview(notice)
        chatPane = ForgeChatPane(frame: content.bounds)
        chatPane.autoresizingMask = [.width, .height]
        chatPane.onActivity = { [weak self] in self?.cancelVoice() }
        chatPane.onClose = { [weak self] in guard let self else { return }; window.makeFirstResponder(web) }
        content.addSubview(chatPane)
        fgs_reset()
        setupPane = ForgeSetupPane(frame: content.bounds)
        setupPane.autoresizingMask = [.width, .height]
        setupPane.enterAction = { [weak self] in self?.enterWorld() }
        setupPane.refreshAction = { [weak self] in
            guard let self else { return }
            if stopping { restartStartup() } else { refreshSetup(verify: false) }
        }
        setupPane.verifyAction = { [weak self] in self?.refreshSetup(verify: true) }
        setupPane.folderAction = { NSWorkspace.shared.open(FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".openagents")) }
        content.addSubview(setupPane)
        if !smoke {
            if launchReport != nil { window.level = .floating }
            window.makeKeyAndOrderFront(nil); NSApp.activate(ignoringOtherApps: true)
        }
    }

    func startChild(_ name: String, _ arguments: [String], resources: URL, fatal: Bool = true) throws -> Process {
        let p = Process()
        p.executableURL = resources.appendingPathComponent(name)
        p.arguments = arguments
        var env = ProcessInfo.processInfo.environment
        for key in Array(env.keys) where key == "PORT" || key.hasPrefix("OA_ROOM_") { env.removeValue(forKey: key) }
        env["COAXIAL_MODE"] = "dev"
        p.environment = env
        let logURL = dataURL.appendingPathComponent("\(name).log")
        fm.createFile(atPath: logURL.path, contents: nil)
        let log = try FileHandle(forWritingTo: logURL); logs.append(log)
        p.standardOutput = log; p.standardError = log
        p.terminationHandler = { [weak self] child in
            Task { @MainActor in
                guard let self, !self.stopping,
                      [self.page, self.room, self.voiceHost].contains(where: { $0 === child }) else { return }
                if fatal { self.fail("\(name) stopped (\(child.terminationStatus)). See \(logURL.path).") }
                else { self.notice.stringValue = "Local voice stopped. See \(logURL.path)."; self.notice.isHidden = false }
            }
        }
        try p.run()
        return p
    }

    func start() async {
        do {
            guard let resources = Bundle.main.resourceURL else { throw ShellError.failed("Missing bundled resources.") }
            if smoke, let report {
                dataURL = report.deletingLastPathComponent().appendingPathComponent("world")
            } else {
                let base = fm.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/aethyrforge", isDirectory: true)
                try fm.createDirectory(at: base, withIntermediateDirectories: true)
                dataURL = base.appendingPathComponent("everglade", isDirectory: true)
                let legacy = fm.homeDirectoryForCurrentUser.appendingPathComponent("Library/Application Support/Aethyrforge/everglade", isDirectory: true)
                if !fm.fileExists(atPath: dataURL.path), fm.fileExists(atPath: legacy.path) {
                    try fm.moveItem(at: legacy, to: dataURL)
                    if (try? fm.contentsOfDirectory(atPath: legacy.deletingLastPathComponent().path).isEmpty) == true { try? fm.removeItem(at: legacy.deletingLastPathComponent()) }
                }
            }
            try fm.createDirectory(at: dataURL, withIntermediateDirectories: true)
            lockFD = Darwin.open(dataURL.appendingPathComponent(".lock").path, O_CREAT | O_RDWR, S_IRUSR | S_IWUSR)
            guard lockFD >= 0, flock(lockFD, LOCK_EX | LOCK_NB) == 0 else {
                throw ShellError.failed("This local world is already open in Aethyrforge.")
            }
            for name in ["world-everglade.json", "room-control.txt"] {
                let destination = dataURL.appendingPathComponent(name)
                if !fm.fileExists(atPath: destination.path) {
                    try fm.copyItem(at: resources.appendingPathComponent("seed/\(name)"), to: destination)
                }
            }
            let r = try reservePort()
            let p: (Int32, Int)
            do { p = try reservePort() } catch { close(r.0); throw error }
            do { voiceReservation = try reservePort(); voicePort = voiceReservation!.1 }
            catch { close(r.0); close(p.0); throw error }
            web.configuration.userContentController.addUserScript(WKUserScript(source: "globalThis.coaxialVoiceHost='ws://127.0.0.1:\(voicePort)';", injectionTime: .atDocumentStart, forMainFrameOnly: true))
            var bytes: [UInt8] = []
            for i in 0..<256 {
                let c = forge_url_byte(Int64(p.1), Int64(r.1), Int64(i))
                if c < 0 { break }; bytes.append(UInt8(c))
            }
            close(r.0); close(p.0)
            guard !bytes.isEmpty, let url = URL(string: String(decoding: bytes, as: UTF8.self)) else { throw ShellError.failed("Invalid local ports.") }
            pageURL = url
            room = try startChild("room", ["--listen", "127.0.0.1", "--port", String(r.1), "--dev", dataURL.path], resources: resources)
            page = try startChild("serve", [resources.appendingPathComponent("web").path, "--host-dir", resources.appendingPathComponent("host").path, "--static", "--cache", "0", "--listen", "127.0.0.1:\(p.1)", "--api", String(r.1)], resources: resources)
            try await waitForHTTP(URL(string: "http://127.0.0.1:\(r.1)/health")!)
            try await waitForHTTP(URL(string: "http://127.0.0.1:\(p.1)/worlds/everglade")!)
            try Data((url.absoluteString + "\n").utf8).write(to: dataURL.appendingPathComponent("local-url.txt"), options: .atomic)
            notice.stringValue = "Loading Everglade…"
            refreshSetup(verify: false)
            web.load(URLRequest(url: url))
            if smoke {
                framePump = Task {
                    while !Task.isCancelled && !stopping {
                        _ = try? await web.evaluateJavaScript("if(globalThis.__forgeFrame){const f=globalThis.__forgeFrame;globalThis.__forgeFrame=null;f(performance.now());} true")
                        try? await Task.sleep(for: .milliseconds(16))
                    }
                }
            }
            try await waitForScene()
            firstFrameSeconds = Date().timeIntervalSince(startedAt)
            notice.isHidden = true
            sceneReady = true; setupPane.sceneReady = true; setupPane.update()
            if let launchReport {
                await inventoryTask?.value
                enterWorld()
                let state = try await launchState()
                try Data(state.utf8).write(to: launchReport, options: .atomic)
                window.level = .normal
                if CommandLine.arguments.contains("--launch-check-retry"), !retryCheckStarted {
                    retryCheckStarted = true
                    Task {
                        room?.terminate()
                        let end = Date().addingTimeInterval(5)
                        while !stopping && Date() < end { try? await Task.sleep(for: .milliseconds(50)) }
                        guard stopping, setupPane.refresh.isEnabled, setupPane.refresh.title == "Retry startup" else {
                            try? Data("Retry control did not become available.".utf8).write(to: launchReport.appendingPathExtension("error")); return
                        }
                        retryCheckPassed = true
                        setupPane.refresh.performClick(nil)
                    }
                }
            }
            if inputSmoke {
                await inventoryTask?.value
                try await inputSmokeCheck()
            } else if voiceSmoke {
                await inventoryTask?.value
                try await voiceSmokeCheck(resources)
            } else if setupSmoke {
                await inventoryTask?.value
                try await setupSmokeCheck()
            } else if smoke { await inventoryTask?.value; setupPane.isHidden = true; try await smokeCheck() }
        } catch { if !stopping { fail(error.localizedDescription) } }
    }

    func waitForHTTP(_ url: URL) async throws {
        let deadline = Date().addingTimeInterval(30)
        while Date() < deadline, !stopping {
            var request = URLRequest(url: url); request.timeoutInterval = 1
            if let (_, response) = try? await URLSession.shared.data(for: request), (response as? HTTPURLResponse)?.statusCode == 200 { return }
            try await Task.sleep(for: .milliseconds(100))
        }
        throw ShellError.failed("Local server did not start. Check room.log and serve.log in the local world folder.")
    }

    func waitForScene() async throws {
        // During retry WKWebView still evaluates in the previous document
        // until navigation commits. Its old ready flag cannot admit this boot.
        let current = "location.origin === 'http://127.0.0.1:\(pageURL.port ?? 0)'"
        var budget: Int64 = 60000
        var previous = ProcessInfo.processInfo.systemUptime
        var wasVisible = false
        while !stopping {
            if let launchReport, let state = try? await launchState() { try? Data(state.utf8).write(to: launchReport, options: .atomic) }
            if let ready = try? await web.evaluateJavaScript("\(current) && globalThis.coaxial?.sceneReady === true"), ready as? Bool == true { return }
            if let message = try? await web.evaluateJavaScript("(\(current) && document.querySelector('#coaxial-load[data-state=fail] .le')?.textContent) || ''"), let s = message as? String, !s.isEmpty { throw ShellError.failed(s) }
            let pageVisible = (try? await web.evaluateJavaScript("!document.hidden")) as? Bool == true
            let visible = smoke || (window.isVisible && !window.isMiniaturized && window.occlusionState.contains(.visible) && pageVisible)
            let now = ProcessInfo.processInfo.systemUptime
            if visible { hiddenSince = nil } else {
                if hiddenSince == nil { hiddenSince = now }
                longestHiddenSeconds = max(longestHiddenSeconds, now - hiddenSince!)
            }
            let elapsed = Int64(max(0, now - previous) * 1000)
            startupPhase = forge_startup_phase(budget, elapsed, visible && wasVisible)
            budget = forge_startup_budget(budget, elapsed, visible && wasVisible)
            previous = now; wasVisible = visible
            if startupPhase == 2 { break }
            try await Task.sleep(for: .milliseconds(200))
        }
        if smoke, let state = try? await web.evaluateJavaScript("JSON.stringify({ready:globalThis.coaxial?.ready,scene:globalThis.coaxial?.sceneReady,gpu:!!navigator.gpu,body:document.body.innerText.slice(0,1200)})") { print("Hidden view: \(state)") }
        throw ShellError.failed("Everglade did not finish its first visible scene frame. Press Retry startup.")
    }
    func launchState() async throws -> String {
        let value = try await web.evaluateJavaScript("JSON.stringify({ready:globalThis.coaxial?.ready,scene:globalThis.coaxial?.sceneReady,frames:globalThis.__forgeFrameCount,hidden:document.hidden,focused:document.hasFocus(),canvas:!!document.querySelector('canvas'),probe:globalThis.coaxial?.probe?.()})")
        let raw = Data((value as? String ?? "{}").utf8)
        var state = (try? JSONSerialization.jsonObject(with: raw)) as? [String: Any] ?? [:]
        state["nativeReady"] = sceneReady; state["enterEnabled"] = setupPane.enter.isEnabled
        state["waitPhase"] = startupPhase
        state["longestHiddenSeconds"] = longestHiddenSeconds
        state["retryCheckPassed"] = retryCheckPassed
        state["setupVisible"] = !setupPane.isHidden; state["failed"] = stopping
        state["firstFrameSeconds"] = firstFrameSeconds
        state["hudHeight"] = chatPane.hud.frame.height; state["windowHeight"] = web.frame.height
        return String(decoding: try JSONSerialization.data(withJSONObject: state, options: [.sortedKeys]), as: UTF8.self)
    }

    // Exercises the actual WKWebView and packaged stack, without ordering a
    // window onto the owner's screen or enabling audio. Isolated world directory.
    func smokeCheck() async throws {
        guard let report else { throw ShellError.failed("--smoke-test needs a report path.") }
        let world = dataURL.appendingPathComponent("world-everglade.json")
        let expected = report.deletingLastPathComponent().appendingPathComponent("expected-world.json")
        let before = try Data(contentsOf: world)
        let reopened = fm.fileExists(atPath: expected.path)
        if reopened {
            guard before == (try Data(contentsOf: expected)) else { throw ShellError.failed("Reopening replaced the saved local world.") }
        } else {
            let old = try JSONSerialization.jsonObject(with: before) as! [String: Any]
            // The first rendered frame can precede the initial world GET.
            try await Task.sleep(for: .seconds(2))
            print("Before input: \(try await web.evaluateJavaScript("coaxial.probe()"))")
            // The seed already has a crate directly ahead at the gate. Walk
            // back along the road so the placement obeys the collision rules.
            _ = try await web.evaluateJavaScript("dispatchEvent(new KeyboardEvent('keydown',{key:'s',code:'KeyS',bubbles:true})); true")
            try await Task.sleep(for: .milliseconds(1200))
            _ = try await web.evaluateJavaScript("dispatchEvent(new KeyboardEvent('keyup',{key:'s',code:'KeyS',bubbles:true})); true")
            print("After walking: \(try await web.evaluateJavaScript("coaxial.probe()"))")
            // The real editor's '1' command places a thing only while EDIT is on.
            let used = try await web.evaluateJavaScript("(()=>{const e=new KeyboardEvent('keydown',{key:'1',code:'Digit1',bubbles:true,cancelable:true});dispatchEvent(e);dispatchEvent(new KeyboardEvent('keyup',{key:'1',code:'Digit1',bubbles:true}));return e.defaultPrevented;})()")
            print("Placement key consumed: \(used)")
            // DONE flushes the existing editor's pending save immediately.
            _ = try await web.evaluateJavaScript("dispatchEvent(new KeyboardEvent('keydown',{key:'b',code:'KeyB',bubbles:true})); dispatchEvent(new KeyboardEvent('keyup',{key:'b',code:'KeyB',bubbles:true})); true")
            let deadline = Date().addingTimeInterval(15)
            var saved = false
            while Date() < deadline {
                try await Task.sleep(for: .milliseconds(200))
                if let bytes = try? Data(contentsOf: world), let doc = try? JSONSerialization.jsonObject(with: bytes) as? [String: Any],
                   let version = doc["version"] as? Int, version > (old["version"] as? Int ?? -1),
                   let edits = (doc["doc"] as? [String: Any])?["edits"] as? String,
                   edits != (old["doc"] as? [String: Any])?["edits"] as? String {
                    try bytes.write(to: expected, options: .atomic); saved = true; break
                }
            }
            if !saved {
                try await captureCanvas(report)
                print("World requests: \(try await web.evaluateJavaScript("JSON.stringify(__forgeRequests)"))")
                throw ShellError.failed("The editor's placed thing was not saved through the local room.")
            }
        }
        let result = try await web.evaluateJavaScript("JSON.stringify({ready:coaxial.ready, scene:coaxial.sceneReady, gpu:!!navigator.gpu, url:location.href, controls:[...document.querySelectorAll('[aria-label]')].map(e=>e.getAttribute('aria-label'))})")
        guard let json = result as? String, var object = try JSONSerialization.jsonObject(with: Data(json.utf8)) as? [String: Any] else { throw ShellError.failed("Missing scene report.") }
        object["savedByHand"] = true; object["reopened"] = reopened; object["firstFrameSeconds"] = firstFrameSeconds
        object["roomPID"] = room?.processIdentifier; object["pagePID"] = page?.processIdentifier
        try await captureCanvas(report)
        try JSONSerialization.data(withJSONObject: object, options: [.prettyPrinted, .sortedKeys]).write(to: report, options: .atomic)
        stopChildren()
        NSApp.terminate(nil)
    }

    func captureCanvas(_ report: URL) async throws {
        if let dataURL = try await web.evaluateJavaScript("document.querySelector('canvas').toDataURL('image/png')") as? String,
           let comma = dataURL.firstIndex(of: ","), let png = Data(base64Encoded: String(dataURL[dataURL.index(after: comma)...])) {
            try png.write(to: report.deletingPathExtension().appendingPathExtension("png"))
        }
    }

    func fail(_ message: String) {
        stopChildren()
        window.level = .normal
        notice.isHidden = false; notice.stringValue = message
        setupPane.errorMessage = message
        setupPane.checking = false; setupPane.verifying = false
        setupPane.isHidden = false
        setupPane.refresh.title = "Retry startup"; setupPane.update()
        if smoke {
            if let report { try? Data("FAILED: \(message)".utf8).write(to: report) }
            fputs("Aethyrforge: \(message)\n", stderr)
            exit(1)
        }
    }
    func restartStartup() {
        stopping = false; sceneReady = false; voiceStartup = nil
        room = nil; page = nil; voiceHost = nil
        chatPane.stopping = false
        if launchReport != nil { window.level = .floating }
        setupPane.sceneReady = false; setupPane.errorMessage = nil
        setupPane.refresh.title = "Refresh"; setupPane.update()
        startup = Task { await start() }
    }

    var settingsURL: URL { dataURL.deletingLastPathComponent().appendingPathComponent("setup.json") }
    var localCLI: URL {
        if let explicit = ProcessInfo.processInfo.environment["OPENAGENTS_CLI"], !explicit.isEmpty { return URL(fileURLWithPath: explicit) }
        if smoke { return Bundle.main.resourceURL!.appendingPathComponent("openagents") }
        return fm.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/bin/openagents")
    }
    func acceptSetup(_ data: Data, kind: Int64) throws {
        guard data.count <= 65536 else { throw ShellError.failed("The setup report is too large.") }
        for (i, byte) in data.enumerated() { _ = fgs_input_byte(Int64(i), Int64(byte)) }
        guard fgs_accept(kind, Int64(data.count)) == 1 else { throw ShellError.failed("The local CLI returned an invalid setup report.") }
    }
    func refreshSetup(verify: Bool) {
        guard !setupPane.verifying && (inventoryTask == nil || !setupPane.checking) else { return }
        setupPane.errorMessage = nil
        setupPane.checking = true; setupPane.verifying = false; setupPane.update()
        inventoryTask = Task {
            do {
                async let version = ForgeCLI.run(localCLI, ["version", "--json"])
                async let agents = ForgeCLI.run(localCLI, ["coder", "agents", "--json"])
                async let models = ForgeCLI.run(localCLI, ["models", "ls", "--json"])
                try acceptSetup(await version, kind: 0)
                try acceptSetup(await agents, kind: 1)
                try acceptSetup(await models, kind: 2)
                if let kept = try? Data(contentsOf: settingsURL) { try? acceptSetup(kept, kind: 3) }
                setupPane.checking = false; setupPane.update()
                if verify && fgs_status(1) > 0 {
                    setupPane.verifying = true; setupPane.update()
                    let verified = try await ForgeCLI.run(localCLI, ["models", "verify", "parakeet", "--json"], timeout: 120)
                    try acceptSetup(verified, kind: 2)
                }
            } catch {
                setupPane.errorMessage = error.localizedDescription
                setupPane.cliStatus.toolTip = localCLI.path
            }
            setupPane.checking = false; setupPane.verifying = false; setupPane.update()
        }
    }
    func enterWorld() {
        guard fgs_can_enter(sceneReady) else { return }
        var bytes: [UInt8] = []
        for i in 0..<512 { let c = fgs_settings_byte(Int64(i)); if c < 0 { break }; bytes.append(UInt8(c)) }
        do {
            try Data(bytes).write(to: settingsURL, options: .atomic)
            setupPane.isHidden = true; window.makeFirstResponder(web)
        } catch { setupPane.errorMessage = "Could not save setup: \(error.localizedDescription)"; setupPane.update() }
    }
    @objc func showSetup() { cancelVoice(); chatPane.closeComposer(); setupPane.isHidden = false; setupPane.sceneReady = sceneReady; setupPane.update() }
    @objc func focusChat() { guard setupPane.isHidden else { return }; chatPane.openComposer() }
    func routeCoderKey(_ event: NSEvent) -> Bool {
        guard event.type == .keyDown || event.type == .keyUp else { return false }
        let down = event.type == .keyDown
        let modified = !event.modifierFlags.intersection([.command, .control, .option]).isEmpty
        let ready = sceneReady && setupPane.isHidden && NSApp.modalWindow == nil && window.attachedSheet == nil
        let realmFocused = (window.firstResponder as? NSView)?.isDescendant(of: web) == true
        let route = fgc_enter_route(ready, chatPane.composerOpen && !realmFocused, Int64(event.keyCode), down, event.isARepeat, modified, coderHeldKeys.contains(event.keyCode))
        if route != 0 {
            if down { coderHeldKeys.insert(event.keyCode) } else { coderHeldKeys.remove(event.keyCode) }
        }
        if route == 2 { focusChat() }
        return route == 1 || route == 2
    }

    // Real AppKit key dispatch over the packaged realm, with no visible window.
    // The staged events separately exercise presentation while the CLI waits.
    func inputSmokeCheck() async throws {
        guard let report else { throw ShellError.failed("--input-smoke needs a report path.") }
        enterWorld()
        func require(_ ok: Bool, _ message: String) throws { if !ok { throw ShellError.failed(message) } }
        func key(_ code: UInt16, down: Bool = true, repeatKey: Bool = false, modifiers: NSEvent.ModifierFlags = []) {
            let text = code == 53 ? "\u{1b}" : "\r"
            let event = NSEvent.keyEvent(with: down ? .keyDown : .keyUp, location: .zero, modifierFlags: modifiers, timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window.windowNumber, context: nil, characters: text, charactersIgnoringModifiers: text, isARepeat: repeatKey, keyCode: code)!
            window.sendEvent(event)
        }
        try require(setupPane.isHidden && chatPane.presentation == 0 && chatPane.hud.isHidden && chatPane.inputScroll.isHidden && chatPane.transcriptScroll.isHidden && chatPane.messagesScroll.isHidden, "Coder must be completely absent at world entry.")
        _ = try await web.evaluateJavaScript("globalThis.__forgeEnterDown=0;globalThis.__forgeEnterUp=0;addEventListener('keydown',e=>{if(e.key==='Enter')__forgeEnterDown++});addEventListener('keyup',e=>{if(e.key==='Enter')__forgeEnterUp++});true")
        for code: UInt16 in [36, 76] {
            key(code)
            try require(chatPane.composerOpen && window.firstResponder === chatPane.input && !chatPane.inputScroll.isHidden && !chatPane.hud.isHidden, "Enter did not open and focus Coder.")
            key(code, repeatKey: true)
            try require(!chatPane.busy, "Holding the opening Enter must not submit a message.")
            key(code, down: false)
            chatPane.input.string = "Retained draft"
            key(53); key(53, down: false)
            try require(!chatPane.composerOpen && chatPane.presentation == 0 && chatPane.input.string == "Retained draft", "Escape must hide Coder and preserve the draft.")
            key(code); key(code, down: false)
            try require(chatPane.input.string == "Retained draft", "Opening Coder lost its draft.")
            // Clicking the world while Coder is open must not give Enter back
            // to region chat; the next press focuses the composer again.
            window.makeFirstResponder(web); key(code); key(code, down: false)
            try require(window.firstResponder === chatPane.input, "Enter leaked after the world regained focus.")
            chatPane.closeComposer()
        }
        chatPane.openComposer(); chatPane.input.string = "line"; chatPane.input.setSelectedRange(NSRange(location: 4, length: 0))
        key(36, modifiers: .shift); key(36, down: false, modifiers: .shift)
        try require(chatPane.input.string == "line\n" && !chatPane.busy, "Shift-Enter must insert a line without submitting.")
        chatPane.input.string = ""; key(36); key(36, down: false)
        try require(chatPane.composerOpen && !chatPane.busy, "An empty draft must stay open.")
        chatPane.busy = true; chatPane.closeComposer()
        chatPane.receive("{\"kind\":\"tool\",\"label\":\"hidden tool activity\"}")
        try require(chatPane.presentation == 0, "Tool metadata must not reveal Coder controls.")
        chatPane.receive("{\"kind\":\"reply\",\"text\":\"Active reply on the left\"}")
        try require(chatPane.presentation == 1 && !chatPane.messagesScroll.isHidden && chatPane.hud.isHidden && chatPane.heading.isHidden && chatPane.status.isHidden && chatPane.inputScroll.isHidden && chatPane.messages.string == "Active reply on the left\n\n", "An active reply must show only message text.")
        try require(chatPane.hitTest(NSPoint(x: 30, y: 500)) == nil, "Closed Coder messages must not block the realm.")
        chatPane.receive("{\"kind\":\"ask\",\"text\":\"Review this request\"}")
        try require(!chatPane.composerOpen && chatPane.allow.isHidden && chatPane.messages.string.contains("Press Enter to review"), "Pending tools must wait for review without opening controls.")
        key(36); key(36, down: false)
        try require(!chatPane.allow.isHidden && !chatPane.deny.isHidden, "Enter must reveal the pending tool's approval controls.")
        key(53); key(53, down: false)
        try require(chatPane.busy && chatPane.pending && !chatPane.composerOpen, "Escape must close without stopping the agent or answering a tool.")
        chatPane.busy = false; chatPane.pending = false; chatPane.update()
        try require(chatPane.presentation == 0 && chatPane.messagesScroll.isHidden, "Completed messages must disappear while Coder is closed.")
        key(36); key(36, down: false)
        try require(chatPane.transcript.string.contains("Active reply on the left"), "Enter must restore conversation history.")
        chatPane.newChat(); chatPane.closeComposer()
        await chatPane.catalogTask?.value
        let root = report.deletingLastPathComponent()
        chatPane.home = root; chatPane.cliOverride = Bundle.main.resourceURL!.appendingPathComponent("openagents")
        chatPane.cwd = root.appendingPathComponent("workspace")
        try fm.createDirectory(at: chatPane.cwd, withIntermediateDirectories: true)
        try fm.createDirectory(at: root.appendingPathComponent(".openagents/aethyrforge"), withIntermediateDirectories: true)
        chatPane.agent.selectItem(at: 5); chatPane.changeAgent(); chatPane.model.stringValue = ""
        key(36); key(36, down: false)
        chatPane.input.string = "FORGE_ENTER_CODER_OK"
        key(36); key(36, repeatKey: true); key(36, down: false)
        try require(chatPane.busy && !chatPane.composerOpen && chatPane.inputScroll.isHidden, "Send must collapse the composer and leave the run active.")
        let deadline = Date().addingTimeInterval(30)
        while chatPane.busy && Date() < deadline { try await Task.sleep(for: .milliseconds(50)) }
        try require(!chatPane.busy && chatPane.lastEnding.hasPrefix("finished") && chatPane.transcript.string.contains("FORGE_ENTER_CODER_OK") && !chatPane.session.isEmpty && chatPane.presentation == 0, "Native Enter did not finish its real Coder CLI round trip.")
        let realm = try await web.evaluateJavaScript("JSON.stringify({downs:__forgeEnterDown,ups:__forgeEnterUp,typing:!!document.querySelector('input[aria-label=\"Typed text\"]'),scene:coaxial.sceneReady})") as? String ?? "{}"
        let state = try JSONSerialization.jsonObject(with: Data(realm.utf8)) as? [String: Any] ?? [:]
        try require(state["downs"] as? Int == 0 && state["ups"] as? Int == 0 && state["typing"] as? Bool == false && state["scene"] as? Bool == true, "Native Enter reached the realm's region chat: \(realm)")
        let result: [String: Any] = ["ok": true, "realm": state, "nativeReturnAndKeypad": true, "heldEnter": true, "shiftEnter": true, "escapeKeepsDraftAndRun": true, "messagesOnlyWhileActive": true, "approvalWaitsForEnter": true, "realCoder": chatPane.session, "ending": chatPane.lastEnding]
        try JSONSerialization.data(withJSONObject: result, options: [.prettyPrinted, .sortedKeys]).write(to: report)
        print("Native Enter / Coder presentation smoke passed")
        stopChildren(); NSApp.terminate(nil)
    }

    // Native input, layout and real CLI round trip, isolated from the live world.
    func chatSmokeCheck() async {
        do {
            guard let report else { throw ShellError.failed("--chat-smoke needs a report path.") }
            setupPane.isHidden = true; notice.isHidden = true
            let root = report.deletingLastPathComponent()
            chatPane.home = root
            chatPane.cliOverride = Bundle.main.resourceURL!.appendingPathComponent("openagents")
            chatPane.cwd = root.appendingPathComponent("workspace")
            try fm.createDirectory(at: chatPane.cwd, withIntermediateDirectories: true)
            try fm.createDirectory(at: root.appendingPathComponent(".openagents/aethyrforge"), withIntermediateDirectories: true)
            let engine = ProcessInfo.processInfo.environment["FORGE_CHAT_SMOKE_ENGINE"] ?? "echo"
            guard let i = (0..<6).first(where: { chatPane.catalog($0, label: false) == engine }) else { throw ShellError.failed("Unknown smoke engine.") }
            chatPane.agent.selectItem(at: i); chatPane.changeAgent(); chatPane.model.stringValue = ""
            if engine == "codex" {
                await chatPane.catalogTask?.value
                if !chatPane.catalogLoaded { chatPane.loadModels(); await chatPane.catalogTask?.value }
                guard chatPane.catalogLoaded else { throw ShellError.failed("Codex model choices did not load through Coder.") }
                chatPane.model.stringValue = chatPane.modelID(0); chatPane.updateEfforts()
                if chatPane.effort.numberOfItems > 1 { chatPane.effort.selectItem(at: 1) }
            }
            window.contentView?.layoutSubtreeIfNeeded()
            guard chatPane.hud.pipeline != nil else { throw ShellError.failed("Aethyr HUD Metal pipeline did not load.") }
            chatPane.input.string = "Remember the token FORGE_CODER_HELIOTROPE. Reply exactly FORGE_CODER_OK. Do not run tools or change files."
            chatPane.submit(); let firstDeadline = Date().addingTimeInterval(120)
            while chatPane.busy && Date() < firstDeadline { try await Task.sleep(for: .milliseconds(50)) }
            guard !chatPane.busy, chatPane.lastEnding.hasPrefix("finished"), chatPane.transcript.string.contains("FORGE_CODER_OK"), !chatPane.session.isEmpty else { throw ShellError.failed("First Coder turn failed: \(chatPane.transcript.string.suffix(1200))") }
            let first = chatPane.session
            func replies(_ id: String) throws -> String {
                let log = try String(contentsOf: chatPane.sessions.appendingPathComponent(id + ".jsonl"), encoding: .utf8)
                return log.split(separator: "\n").compactMap { line -> String? in
                    guard let record = try? JSONSerialization.jsonObject(with: Data(line.utf8)) as? [String: Any], record["kind"] as? String == "reply" else { return nil }
                    return record["text"] as? String
                }.joined(separator: "\n")
            }
            guard try replies(first).contains("FORGE_CODER_OK") else { throw ShellError.failed("The provider did not reply to the first turn.") }
            chatPane.input.string = "What token did I ask you to remember? Reply with only that token. Do not run tools."
            chatPane.submit(); let secondDeadline = Date().addingTimeInterval(120)
            while chatPane.busy && Date() < secondDeadline { try await Task.sleep(for: .milliseconds(50)) }
            guard !chatPane.busy, chatPane.lastEnding.hasPrefix("finished"), chatPane.session != first else { throw ShellError.failed("Follow-up Coder turn failed.") }
            let log = try String(contentsOf: chatPane.sessions.appendingPathComponent(chatPane.session + ".jsonl"), encoding: .utf8)
            guard log.contains("\"parent\":\"\(first)\""), try replies(chatPane.session).contains("FORGE_CODER_HELIOTROPE") else { throw ShellError.failed("Coder did not carry the previous turn.") }
            let followup = chatPane.session
            var approved = false
            var stopped = false
            if ProcessInfo.processInfo.environment["FORGE_CHAT_SMOKE_ACTIONS"] == "1" {
                chatPane.input.string = "Use a shell command to write exactly HUD_APPROVED to hud-approved.txt in the working directory. Do not write any other file."
                chatPane.submit(); let end = Date().addingTimeInterval(120)
                while chatPane.busy && Date() < end {
                    if chatPane.pending { chatPane.approve(); approved = true }
                    try await Task.sleep(for: .milliseconds(50))
                }
                guard !chatPane.busy, approved, (try? String(contentsOf: chatPane.cwd.appendingPathComponent("hud-approved.txt"), encoding: .utf8))?.trimmingCharacters(in: .whitespacesAndNewlines) == "HUD_APPROVED" else { throw ShellError.failed("Coder approval/write flow failed: \(chatPane.transcript.string.suffix(1200))") }
                chatPane.input.string = "Run the shell command sleep 60, then reply DONE."
                chatPane.submit(); let stopEnd = Date().addingTimeInterval(30)
                while chatPane.busy && chatPane.session.isEmpty && Date() < stopEnd { try await Task.sleep(for: .milliseconds(50)) }
                chatPane.stopTurn()
                while chatPane.busy && Date() < stopEnd { try await Task.sleep(for: .milliseconds(50)) }
                stopped = !chatPane.busy && chatPane.lastEnding.hasPrefix("stopped")
                guard stopped else { throw ShellError.failed("Coder did not stop the active turn.") }
            }
            let result: [String: Any] = ["ok": true, "engine": engine, "model": chatPane.model.stringValue, "effort": chatPane.effort.titleOfSelectedItem ?? "", "first": first, "followup": followup, "ending": chatPane.lastEnding, "approval": approved, "stop": stopped, "hud": "aethyr/ui/draw.coax", "cli": chatPane.cli.path]
            try JSONSerialization.data(withJSONObject: result, options: [.prettyPrinted, .sortedKeys]).write(to: report)
            print("Coder HUD smoke passed: \(engine)")
        } catch { if let report { try? Data(error.localizedDescription.utf8).write(to: report.appendingPathExtension("error")) }; print(error.localizedDescription) }
        chatPane.shutdown(); NSApp.terminate(nil)
    }

    func ensureVoiceHost() async throws {
        if let voiceStartup { try await voiceStartup.value; return }
        if voiceHost?.isRunning == true { return }
        let task = Task {
            guard let resources = Bundle.main.resourceURL, !setupText(6).isEmpty else { throw ShellError.failed("The shared Parakeet model is unavailable.") }
            if let reservation = voiceReservation { close(reservation.0); voiceReservation = nil }
            voiceHost = try startChild("voice-host", ["--listen", "127.0.0.1:\(voicePort)", "--provider", "local", "--recognizer", "parakeet", "--synthesizer", "local", "--agent", "echo", "--parakeet-model", setupText(6), "--parakeet-device", "gpu"], resources: resources, fatal: false)
            try await waitForHTTP(URL(string: "http://127.0.0.1:\(voicePort)/health")!)
        }
        voiceStartup = task
        defer { voiceStartup = nil }
        try await task.value
    }
    func cancelVoice() { _ = web?.evaluateJavaScript("globalThis.coaxialVoiceCancel?.(); true") }
    func applicationDidResignActive(_ notification: Notification) { cancelVoice() }
    func windowDidResignKey(_ notification: Notification) { coderHeldKeys.removeAll(); cancelVoice() }
    func windowDidMiniaturize(_ notification: Notification) { cancelVoice() }
    func traceVoice(_ line: String) {
        guard let dataURL, line.utf8.count <= 1024 else { return }
        let path = dataURL.appendingPathComponent("voice-access.log")
        if !fm.fileExists(atPath: path.path) { fm.createFile(atPath: path.path, contents: nil) }
        if let file = try? FileHandle(forWritingTo: path) { defer { try? file.close() }; _ = try? file.seekToEnd(); try? file.write(contentsOf: Data((line + "\n").utf8)) }
    }
    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
        if message.frameInfo.isMainFrame, let line = message.body as? String { traceVoice(line) }
    }

    func voiceSmokeCheck(_ resources: URL) async throws {
        guard let report, voiceHost == nil else { throw ShellError.failed("Voice loaded before it was requested.") }
        try await ensureVoiceHost()
        let fixture = try Data(contentsOf: resources.appendingPathComponent("hello-foreman.wav")).base64EncodedString()
        _ = try await web.evaluateJavaScript("""
        globalThis.__voiceCheck = []; globalThis.__voiceError = ''; globalThis.__voiceSockets = 0;
        globalThis.__voiceTest = (async () => {
          const {startVoice} = await import('/coaxial/voice.js');
          const W = WebSocket;
          globalThis.WebSocket = class extends W { constructor(...a) { super(...a); if(String(a[0]) === coaxialVoiceHost) __voiceSockets++; } };
          const md = navigator.mediaDevices, original = md.getUserMedia.bind(md);
          const fixtureContext = new AudioContext(); await fixtureContext.resume();
          globalThis.__voiceFixtureContext = fixtureContext;
          const dest = fixtureContext.createMediaStreamDestination();
          let deliver;
          md.getUserMedia = () => new Promise(r => {deliver = r});
          const cancelled = startVoice({send:()=>{},route:'everglade',wasmUrl:'/build/voice.wasm'});
          const starting = cancelled.toggle('on'); cancelled.toggle(false); deliver(dest.stream);
          await starting;
          globalThis.__voiceCancelled = !cancelled.running() && dest.stream.getTracks().every(t=>t.readyState==='ended') && __voiceSockets===0;
          globalThis.WebSocket = W;
          const source = fixtureContext.createBufferSource();
          const wav = Uint8Array.from(atob('\(fixture)'), c=>c.charCodeAt(0));
          source.buffer = await fixtureContext.decodeAudioData(wav.buffer);
          const capture = fixtureContext.createMediaStreamDestination(); source.connect(capture);
          md.getUserMedia = async () => capture.stream;
          globalThis.__voiceFixture = source;
          globalThis.__voiceClient = startVoice({send:(k,t)=>__voiceCheck.push(t),route:'everglade',wasmUrl:'/build/voice.wasm'});
          __voiceClient.face('Everglade builder'); await __voiceClient.toggle('on');
          globalThis.__voiceRestore = () => {md.getUserMedia=original;};
        })().catch(e=>{globalThis.__voiceError=String(e)}); true;
        """)
        let listenDeadline = Date().addingTimeInterval(15)
        var listening = false
        while Date() < listenDeadline {
            if try await web.evaluateJavaScript("__voiceCheck.some(t=>t.startsWith('listening|'))") as? Bool == true { listening = true; break }
            try await Task.sleep(for: .milliseconds(100))
        }
        guard listening else { throw ShellError.failed("Browser voice did not listen: \(String(describing: try await web.evaluateJavaScript("__voiceError || JSON.stringify(__voiceCheck)")))") }
        _ = try await web.evaluateJavaScript("__voiceFixture.start(); true")
        try await Task.sleep(for: .seconds(4.2))
        _ = try await web.evaluateJavaScript("__voiceClient.toggle('end'); true")
        let finalDeadline = Date().addingTimeInterval(30)
        var transcript = ""
        while Date() < finalDeadline {
            transcript = try await web.evaluateJavaScript("__voiceCheck.find(t=>t.startsWith('heard|'))?.slice(6) || ''") as? String ?? ""
            if !transcript.isEmpty { break }
            try await Task.sleep(for: .milliseconds(100))
        }
        let cancelled = try await web.evaluateJavaScript("__voiceCancelled === true") as? Bool == true
        _ = try await web.evaluateJavaScript("__voiceClient.toggle(false); __voiceRestore?.(); __voiceFixtureContext.close(); true")
        guard cancelled, transcript.lowercased().contains("hello foreman"), transcript.lowercased().contains("building today") else {
            throw ShellError.failed("The local browser voice fixture failed: \(transcript). Cancellation: \(cancelled).")
        }
        let result: [String: Any] = ["local": true, "transcript": transcript, "lateMicrophoneCancelled": cancelled,
            "voicePID": voiceHost!.processIdentifier, "roomPID": room!.processIdentifier, "pagePID": page!.processIdentifier,
            "startupModelLoaded": false]
        try JSONSerialization.data(withJSONObject: result, options: [.prettyPrinted, .sortedKeys]).write(to: report, options: .atomic)
        stopChildren(); NSApp.terminate(nil)
    }

    func setupSmokeCheck() async throws {
        guard let report, fgs_can_enter(sceneReady), fgs_status(1) == 1 else { throw ShellError.failed("Setup did not discover the CLI and installed shared voice model without hashing.") }
        guard !setupPane.isHidden, setupPane.enter.isEnabled else { throw ShellError.failed("Setup did not appear over the running world.") }
        enterWorld()
        guard setupPane.isHidden, fm.fileExists(atPath: settingsURL.path) else { throw ShellError.failed("Setup could not enter the world and keep its choices.") }
        showSetup()
        let result: [String: Any] = ["ready": sceneReady, "cli": fgs_status(0) == 1, "voiceInstalled": fgs_status(1) > 0, "voiceVerified": fgs_status(1) == 2,
            "localAgents": fgs_count(-1), "selectedAgent": fgs_selected(), "voicePath": setupText(6),
            "setupVisible": !setupPane.isHidden, "settingsSaved": true, "firstFrameSeconds": firstFrameSeconds,
            "roomPID": room!.processIdentifier, "pagePID": page!.processIdentifier]
        try await captureCanvas(report)
        try JSONSerialization.data(withJSONObject: result, options: [.prettyPrinted, .sortedKeys]).write(to: report, options: .atomic)
        stopChildren(); NSApp.terminate(nil)
    }

    @objc func reloadWorld() {
        guard !stopping, let pageURL else { return }
        web.load(URLRequest(url: pageURL))
    }
    @objc func openWorldFolder() { if let dataURL { NSWorkspace.shared.open(dataURL) } }
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
    func applicationWillTerminate(_ notification: Notification) { startup?.cancel(); stopChildren() }
    func stopChildren() {
        stopping = true
        chatPane?.shutdown()
        framePump?.cancel()
        inventoryTask?.cancel()
        voiceStartup?.cancel()
        if let reservation = voiceReservation { close(reservation.0); voiceReservation = nil }
        for child in [page, room, voiceHost].compactMap({ $0 }) where child.isRunning { child.terminate() }
        // Bounded shutdown; reap before releasing the single-world lock.
        let end = Date().addingTimeInterval(2)
        while [page, room, voiceHost].compactMap({ $0 }).contains(where: { $0.isRunning }) && Date() < end { usleep(10_000) }
        for child in [page, room, voiceHost].compactMap({ $0 }) where child.isRunning { kill(child.processIdentifier, SIGKILL) }
        for child in [page, room, voiceHost].compactMap({ $0 }) { child.waitUntilExit() }
        for log in logs { try? log.close() }; logs.removeAll()
        if lockFD >= 0 { flock(lockFD, LOCK_UN); close(lockFD); lockFD = -1 }
    }

    func webView(_ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction, decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
        let url = navigationAction.request.url
        decisionHandler(url?.scheme == "http" && url?.host == "127.0.0.1" && url?.port == pageURL?.port ? .allow : .cancel)
    }
    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
        if !stopping && (error as NSError).code != NSURLErrorCancelled { fail(error.localizedDescription) }
    }
    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) { fail("The world view stopped. Quit and reopen Aethyrforge.") }
    func webView(_ webView: WKWebView, requestMediaCapturePermissionFor origin: WKSecurityOrigin, initiatedByFrame frame: WKFrameInfo, type: WKMediaCaptureType, decisionHandler: @escaping (WKPermissionDecision) -> Void) {
        traceVoice("microphone request: main=\(frame.isMainFrame) local=\(origin.host == "127.0.0.1" && origin.port == pageURL?.port) key=\(window.isKeyWindow) setup=\(!setupPane.isHidden) authorization=\(AVCaptureDevice.authorizationStatus(for: .audio).rawValue)")
        guard type == .microphone, frame.isMainFrame, origin.protocol == "http", origin.host == "127.0.0.1", origin.port == pageURL?.port,
              !smoke, !stopping, setupPane.isHidden, window.isKeyWindow else { decisionHandler(.deny); return }
        Task {
            let allowed = await AVCaptureDevice.requestAccess(for: .audio)
            traceVoice("microphone permission: \(allowed)")
            guard allowed, !stopping else { decisionHandler(.deny); return }
            do { try await ensureVoiceHost(); decisionHandler(.grant) }
            catch { notice.stringValue = "Local voice: \(error.localizedDescription)"; notice.isHidden = false; decisionHandler(.deny) }
        }
    }
}

MainActor.assumeIsolated {
    let app = NSApplication.shared
    let delegate = ForgeApp()
    app.delegate = delegate
    app.run()
    withExtendedLifetime(delegate) {}
}
