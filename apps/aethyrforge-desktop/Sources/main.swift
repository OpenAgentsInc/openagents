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
final class ForgeApp: NSObject, NSApplicationDelegate, NSWindowDelegate, WKNavigationDelegate, WKUIDelegate, WKScriptMessageHandler {
    let fm = FileManager.default
    var window: NSWindow!
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
    var sceneReady = false
    var dataURL: URL!
    var pageURL: URL!
    var lockFD: Int32 = -1
    var stopping = false
    let startedAt = Date()
    var firstFrameSeconds = 0.0
    let setupSmoke = CommandLine.arguments.contains("--setup-smoke")
    let voiceSmoke = CommandLine.arguments.contains("--voice-smoke")
    var smoke: Bool { setupSmoke || voiceSmoke || CommandLine.arguments.contains("--smoke-test") }
    var report: URL? {
        guard let i = CommandLine.arguments.firstIndex(of: voiceSmoke ? "--voice-smoke" : setupSmoke ? "--setup-smoke" : "--smoke-test"), i + 1 < CommandLine.arguments.count else { return nil }
        return URL(fileURLWithPath: CommandLine.arguments[i + 1])
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(smoke ? .prohibited : .regular)
        makeWindow()
        startup = Task { await start() }
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
        let folder = view.addItem(withTitle: "Open Local World Folder", action: #selector(openWorldFolder), keyEquivalent: "o"); folder.target = self
        folder.keyEquivalentModifierMask = [.command, .shift]
        NSApp.mainMenu = menu

        let config = WKWebViewConfiguration()
        config.userContentController.add(self, name: "forgeVoiceStatus")
        config.websiteDataStore = .nonPersistent()
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
        window = NSWindow(contentRect: web.frame, styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
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
        fgs_reset()
        setupPane = ForgeSetupPane(frame: content.bounds)
        setupPane.autoresizingMask = [.width, .height]
        setupPane.enterAction = { [weak self] in self?.enterWorld() }
        setupPane.refreshAction = { [weak self] in self?.refreshSetup(verify: false) }
        setupPane.verifyAction = { [weak self] in self?.refreshSetup(verify: true) }
        setupPane.folderAction = { NSWorkspace.shared.open(FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".openagents")) }
        content.addSubview(setupPane)
        if !smoke { window.makeKeyAndOrderFront(nil); NSApp.activate(ignoringOtherApps: true) }
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
                guard let self, !self.stopping else { return }
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
            if voiceSmoke {
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
        let deadline = Date().addingTimeInterval(60)
        while Date() < deadline, !stopping {
            if let ready = try? await web.evaluateJavaScript("globalThis.coaxial?.sceneReady === true"), ready as? Bool == true { return }
            if let message = try? await web.evaluateJavaScript("document.querySelector('#coaxial-load[data-state=fail] .le')?.textContent || ''"), let s = message as? String, !s.isEmpty { throw ShellError.failed(s) }
            try await Task.sleep(for: .milliseconds(200))
        }
        if smoke, let state = try? await web.evaluateJavaScript("JSON.stringify({ready:globalThis.coaxial?.ready,scene:globalThis.coaxial?.sceneReady,gpu:!!navigator.gpu,body:document.body.innerText.slice(0,1200)})") { print("Hidden view: \(state)") }
        throw ShellError.failed("Everglade did not draw its first frame. Quit and reopen Aethyrforge to retry.")
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
        notice.isHidden = false; notice.stringValue = message
        if smoke {
            if let report { try? Data("FAILED: \(message)".utf8).write(to: report) }
            fputs("Aethyrforge: \(message)\n", stderr)
            exit(1)
        }
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
    @objc func showSetup() { cancelVoice(); setupPane.isHidden = false; setupPane.sceneReady = sceneReady; setupPane.update() }

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
    func windowDidResignKey(_ notification: Notification) { cancelVoice() }
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
