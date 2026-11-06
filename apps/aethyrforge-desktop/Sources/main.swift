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
    override var title: String {
        get { super.title }
        set {
            let args = CommandLine.arguments
            if let i = args.firstIndex(of: "--test-window-title"), i+1 < args.count { super.title = args[i+1] + " · " + newValue }
            else { super.title = newValue }
        }
    }
    var coderKey: ((NSEvent) -> Bool)?
    override func sendEvent(_ event: NSEvent) {
        if coderKey?(event) == true { return }
        super.sendEvent(event)
    }
}

@MainActor
final class ForgeApp: NSObject, NSApplicationDelegate, NSWindowDelegate, NSMenuItemValidation, WKNavigationDelegate, WKUIDelegate, WKScriptMessageHandler, WKScriptMessageHandlerWithReply {
    let fm = FileManager.default
    var window: ForgeWindow!
    var coderHeldKeys: Set<UInt16> = []
    var web: WKWebView!
    var metalWorld: ForgeMetalWorld?
    var metalLocalOrigin = ""
    var previewSwitchTask: Task<Void, Never>?
    var previewMenu: NSMenuItem?
    var lastPreviewRetired: [String: Any] = [:]
    var worldBackend: Int64 {
        let a = CommandLine.arguments
        if let i = a.firstIndex(of: "--world"), i+1 == a.count { return -1 }
        let name = a.firstIndex(of: "--world").flatMap { $0+1<a.count ? a[$0+1]:nil } ?? ""
        return Data(name.utf8).withUnsafeBytes { forge_world_backend_bytes($0.bindMemory(to: UInt8.self).baseAddress, Int64($0.count)) }
    }
    var nativeWorld: Bool { worldBackend == 1 }
    var worldView: NSView { metalWorld ?? web }
    var notice: NSTextField!
    var room: Process?
    var page: Process?
    var voiceHost: Process?
    var voiceStartup: Task<Void, Error>?
    var voiceWarmup: Task<Void, Error>?
    var voiceWarmReady = false
    var voiceWarmEpoch = 0
    var voiceReservation: (Int32, Int)?
    var voicePort = 0
    var microphonePrompt = false
    var microphonePermissionTask: Task<Bool, Never>?
    // SDK transports may be replaced only by the isolated setup smoke.
    var microphoneAuthorization: (() -> AVAuthorizationStatus)?
    var microphoneAccessRequest: (() async -> Bool)?
    var voiceCaptureRequest: Int?
    var voiceCaptureEpoch = 0
    var microphoneDecisions = 0
    var microphoneCheckFailStartup = false
    var microphoneCheckDelay = false
    var logs: [FileHandle] = []
    var startup: Task<Void, Never>?
    var terminationSignal: DispatchSourceSignal?
    var framePump: Task<Void, Never>?
    var inventoryTask: Task<Void, Never>?
    var sceneContextTask: Task<Void, Never>?
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
    var highRefreshEnabled = false
    var worldPauseWanted: Bool?
    let setupSmoke = CommandLine.arguments.contains("--setup-smoke")
    let voiceSmoke = CommandLine.arguments.contains("--voice-smoke")
    let voiceWarmSmoke = CommandLine.arguments.contains("--voice-warm-smoke")
    let chatSmoke = CommandLine.arguments.contains("--chat-smoke")
    let inputSmoke = CommandLine.arguments.contains("--input-smoke")
    let hotbarSmoke = CommandLine.arguments.contains("--hotbar-smoke")
    let sceneSmoke = CommandLine.arguments.contains("--scene-smoke")
    let npcSmoke = CommandLine.arguments.contains("--npc-smoke")
    let audioSmoke = CommandLine.arguments.contains("--audio-smoke")
    var microphoneReport: URL? {
        guard let i = CommandLine.arguments.firstIndex(of: "--microphone-check"), i + 1 < CommandLine.arguments.count else { return nil }
        return URL(fileURLWithPath: CommandLine.arguments[i + 1])
    }
    var launchReport: URL? {
        guard let i = CommandLine.arguments.firstIndex(of: "--launch-check"), i + 1 < CommandLine.arguments.count else { return nil }
        return URL(fileURLWithPath: CommandLine.arguments[i + 1])
    }
    var frameProfileURL: URL? {
        guard let i = CommandLine.arguments.firstIndex(of: "--frame-profile"), i + 1 < CommandLine.arguments.count else { return nil }
        return URL(fileURLWithPath: CommandLine.arguments[i + 1])
    }
    var smoke: Bool { hotbarSmoke || voiceWarmSmoke || audioSmoke || npcSmoke || sceneSmoke || inputSmoke || chatSmoke || setupSmoke || voiceSmoke || CommandLine.arguments.contains("--smoke-test") }
    var report: URL? {
        guard let i = CommandLine.arguments.firstIndex(of: hotbarSmoke ? "--hotbar-smoke" : voiceWarmSmoke ? "--voice-warm-smoke" : audioSmoke ? "--audio-smoke" : npcSmoke ? "--npc-smoke" : sceneSmoke ? "--scene-smoke" : inputSmoke ? "--input-smoke" : chatSmoke ? "--chat-smoke" : voiceSmoke ? "--voice-smoke" : setupSmoke ? "--setup-smoke" : "--smoke-test"), i + 1 < CommandLine.arguments.count else { return nil }
        return URL(fileURLWithPath: CommandLine.arguments[i + 1])
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        guard worldBackend >= 0 else { fputs("Aethyrforge: --world requires web or metal.\n",stderr); NSApp.terminate(nil); return }
        signal(SIGTERM, SIG_IGN)
        terminationSignal = DispatchSource.makeSignalSource(signal: SIGTERM, queue: .main)
        terminationSignal?.setEventHandler { NSApp.terminate(nil) }
        terminationSignal?.resume()
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
        previewMenu = view.addItem(withTitle: previewTitle(), action: #selector(toggleWebPreview), keyEquivalent: "")
        previewMenu?.target = self
        let setup = view.addItem(withTitle: "Setup…", action: #selector(showSetup), keyEquivalent: ","); setup.target = self
        let status = view.addItem(withTitle: "Status…", action: #selector(showStatus), keyEquivalent: "i"); status.target = self
        let chat = view.addItem(withTitle: "Message Coding Agent", action: #selector(focusChat), keyEquivalent: "l"); chat.target = self
        let folder = view.addItem(withTitle: "Open Local World Folder", action: #selector(openWorldFolder), keyEquivalent: "o"); folder.target = self
        folder.keyEquivalentModifierMask = [.command, .shift]
        let windowItem = NSMenuItem(); menu.addItem(windowItem)
        let windowMenu = NSMenu(title: "Window"); windowItem.submenu = windowMenu
        windowMenu.addItem(withTitle: "Minimize", action: #selector(NSWindow.performMiniaturize(_:)), keyEquivalent: "m")
        NSApp.windowsMenu = windowMenu
        NSApp.mainMenu = menu

        let config = WKWebViewConfiguration()
        if benchmarkReportURL != nil,let resources=Bundle.main.resourceURL,
            let script=try? String(contentsOf:resources.appendingPathComponent("host/everglade-benchmark.js"),encoding:.utf8) {
            let gpuTimestamps = !CommandLine.arguments.contains("--bench-no-timestamps")
            config.userContentController.addUserScript(WKUserScript(source:"globalThis.coaxialBenchmarkOptions={mode:'replay',fixture:'/build/bench/raid.json',timestamps:\(gpuTimestamps)};\n"+script,injectionTime:.atDocumentStart,forMainFrameOnly:true))
        }
        let displayHz = NSScreen.main?.maximumFramesPerSecond ?? 60
        if forge_high_refresh(Int64(displayHz)) { highRefreshEnabled = configureHighRefresh(config.preferences) }
        let preferredHz = benchmarkReportURL != nil ? 60 : highRefreshEnabled ? displayHz : 60
        config.userContentController.addUserScript(WKUserScript(source: "globalThis.coaxialPreferredFrameRate=\(preferredHz);globalThis.coaxialMinimumFrameRate=60;", injectionTime: .atDocumentStart, forMainFrameOnly: true))
        if frameProfileURL != nil { config.userContentController.addUserScript(WKUserScript(source: "globalThis.coaxialFrameTrace=true;", injectionTime: .atDocumentStart, forMainFrameOnly: true)) }
        config.userContentController.add(self, name: "forgeVoiceStatus")
        config.userContentController.add(self, name: "forgeVoiceCoder")
        config.userContentController.add(self, name: "forgeMetalMessage")
        config.userContentController.addScriptMessageHandler(self, contentWorld: .page, name: "forgeVoiceCapture")
        if !smoke || voiceSmoke || voiceWarmSmoke {
            config.userContentController.addUserScript(WKUserScript(source: """
            globalThis.coaxialVoicePrepareCapture = capture => webkit.messageHandlers.forgeVoiceCapture.postMessage({schema:'aethyrforge.microphone-capture.v1',kind:'prepare',capture});
            globalThis.coaxialVoiceCancelCapture = capture => webkit.messageHandlers.forgeVoiceCapture.postMessage({schema:'aethyrforge.microphone-capture.v1',kind:'cancel',capture});
            """, injectionTime: .atDocumentStart, forMainFrameOnly: true))
        }
        config.userContentController.addUserScript(WKUserScript(source: """
        globalThis.coaxialVoiceFinal = e => {
          webkit.messageHandlers.forgeVoiceCoder.postMessage(JSON.stringify({schema:'aethyrforge.voice-coder.v1',kind:'final',ticket:e.ticket,text:e.text}));
          return true;
        };
        globalThis.coaxialVoiceStopped = ticket => webkit.messageHandlers.forgeVoiceCoder.postMessage(JSON.stringify({schema:'aethyrforge.voice-coder.v1',kind:'cancel',ticket}));
        """, injectionTime: .atDocumentStart, forMainFrameOnly: true))
        config.websiteDataStore = .nonPersistent()
        if offlineReportURL != nil {
            config.userContentController.addUserScript(WKUserScript(source:"globalThis.__forgeRoomWorkers=0;const NativeWorker=Worker;globalThis.Worker=class extends NativeWorker{constructor(...args){super(...args);__forgeRoomWorkers++;}};",injectionTime:.atDocumentStart,forMainFrameOnly:true))
        }
        if setupProfileURL != nil {
            config.userContentController.addUserScript(WKUserScript(source:"globalThis.__forgeErrors=[];addEventListener('error',e=>__forgeErrors.push(String(e.message)));addEventListener('unhandledrejection',e=>__forgeErrors.push(String(e.reason)));",injectionTime:.atDocumentStart,forMainFrameOnly:true))
        }
        if launchReport != nil {
            config.userContentController.addUserScript(WKUserScript(source: "globalThis.__forgeFrameCount=0;const nativeRAF=requestAnimationFrame;globalThis.requestAnimationFrame=f=>nativeRAF(t=>{__forgeFrameCount++;f(t);});", injectionTime: .atDocumentStart, forMainFrameOnly: true))
        }
        config.mediaTypesRequiringUserActionForPlayback = []
        configureAudioOutput(config, muted: smoke || CommandLine.arguments.contains("--mute-audio") || launchReport != nil || microphoneReport != nil || frameProfileURL != nil || setupProfileURL != nil || offlineReportURL != nil)
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
        if nativeWorld {
            web.isHidden = true
            let view = makeMetalWorld(frame: content.bounds)
            view.preferredHz = Float(preferredHz); metalWorld = view; content.addSubview(view)
        }
        window.contentView = content; window.center()
        if benchmarkReportURL != nil {window.minSize=NSSize(width:900,height:500);window.setContentSize(NSSize(width:1280,height:720))}
        notice = NSTextField(labelWithString: "Starting local Everglade…")
        notice.textColor = .white; notice.backgroundColor = .black; notice.drawsBackground = true
        notice.alignment = .center; notice.frame = NSRect(x: 0, y: 0, width: 1280, height: 28)
        notice.autoresizingMask = [.width, .maxYMargin]
        content.addSubview(notice)
        chatPane = ForgeChatPane(frame: content.bounds)
        chatPane.autoresizingMask = [.width, .height]
        chatPane.onActivity = { [weak self] in self?.cancelVoice("coder-activity") }
        chatPane.onClose = { [weak self] in guard let self else { return }; window.makeFirstResponder(worldView) }
        content.addSubview(chatPane)
        fgs_reset()
        if let saved = try? Data(contentsOf: settingsURL) { try? acceptSetup(saved, kind: 3) }
        setupPane = ForgeSetupPane(frame: content.bounds)
        setupPane.isHidden = fgs_completed()
        setupPane.autoresizingMask = [.width, .height]
        setupPane.enterAction = { [weak self] in self?.enterWorld() }
        setupPane.refreshAction = { [weak self] in
            guard let self else { return }
            if stopping { restartStartup() } else { refreshSetup(verify: false) }
        }
        setupPane.verifyAction = { [weak self] in self?.refreshSetup(verify: true) }
        setupPane.microphoneAction = { [weak self] in self?.microphoneSetupPressed() }
        setupPane.folderAction = { NSWorkspace.shared.open(FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".openagents")) }
        content.addSubview(setupPane)
        if !smoke {
            if launchReport != nil || microphoneReport != nil { window.level = .floating }
            window.makeKeyAndOrderFront(nil); NSApp.activate(ignoringOtherApps: true)
        }
        refreshMicrophoneSetup()
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
        if name == "voice-host", fm.fileExists(atPath: logURL.path) {
            let previous = dataURL.appendingPathComponent("voice-host.previous.log")
            try? fm.removeItem(at: previous); try? fm.moveItem(at: logURL, to: previous)
        }
        fm.createFile(atPath: logURL.path, contents: nil)
        let log = try FileHandle(forWritingTo: logURL); logs.append(log)
        p.standardOutput = log; p.standardError = log
        p.terminationHandler = { [weak self] child in
            Task { @MainActor in
                guard let self, !self.stopping,
                      [self.page, self.room, self.voiceHost].contains(where: { $0 === child }) else { return }
                if fatal { self.fail("\(name) stopped (\(child.terminationStatus)). See \(logURL.path).") }
                else { self.voiceWarmReady = false; fgs_voice_ready(3); self.setupPane.update(); self.notice.stringValue = "Local voice stopped. See \(logURL.path)."; self.notice.isHidden = false }
            }
        }
        try p.run()
        return p
    }
    // This experimental, ad-hoc signed Mac shell uses WebKit's feature SPI.
    // Enumerate and check availability; never change Safari or system defaults.
    func configureHighRefresh(_ preferences: WKPreferences) -> Bool {
        let kind: AnyObject = WKPreferences.self
        let featuresSelector = NSSelectorFromString("_features")
        let setterSelector = NSSelectorFromString("_setEnabled:forFeature:")
        let getterSelector = NSSelectorFromString("_isEnabledForFeature:")
        guard kind.responds(to: featuresSelector), preferences.responds(to: setterSelector), preferences.responds(to: getterSelector),
              let features = kind.perform(featuresSelector)?.takeUnretainedValue() as? [NSObject],
              let feature = features.first(where: { $0.value(forKey: "key") as? String == "PreferPageRenderingUpdatesNear60FPSEnabled" }) else { return false }
        let set = unsafeBitCast(preferences.method(for: setterSelector), to: (@convention(c) (AnyObject, Selector, Bool, AnyObject) -> Void).self)
        let get = unsafeBitCast(preferences.method(for: getterSelector), to: (@convention(c) (AnyObject, Selector, AnyObject) -> Bool).self)
        set(preferences, setterSelector, false, feature)
        return !get(preferences, getterSelector, feature)
    }

    func start() async {
        do {
            guard let resources = Bundle.main.resourceURL else { throw ShellError.failed("Missing bundled resources.") }
            if let i = CommandLine.arguments.firstIndex(of: "--data-dir"), i+1 < CommandLine.arguments.count {
                dataURL = URL(fileURLWithPath: CommandLine.arguments[i+1], isDirectory: true)
            } else if let report = smoke ? report : microphoneReport ?? setupProfileURL ?? offlineReportURL ?? benchmarkReportURL {
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
            if !offlineWorker {
                web.configuration.userContentController.addUserScript(WKUserScript(source: "globalThis.coaxialNativeRoomHost='ws://127.0.0.1:\(r.1)';", injectionTime: .atDocumentStart, forMainFrameOnly: true))
            }
            var bytes: [UInt8] = []
            for i in 0..<256 {
                let c = benchmarkReportURL != nil ? forge_benchmark_url_byte(Int64(p.1),Int64(i)) : offlineMode ? forge_offline_url_byte(Int64(p.1),Int64(i)) : nativeWorld ? forge_native_url_byte(Int64(p.1),Int64(i)) : forge_url_byte(Int64(p.1), Int64(r.1), Int64(i))
                if c < 0 { break }; bytes.append(UInt8(c))
            }
            close(r.0); close(p.0)
            guard !bytes.isEmpty, let url = URL(string: String(decoding: bytes, as: UTF8.self)) else { throw ShellError.failed("Invalid local ports.") }
            pageURL = url
            room = try startChild("room", ["--listen", "127.0.0.1", "--port", String(r.1), "--dev", dataURL.path, "--native-local"], resources: resources)
            page = try startChild("serve", [resources.appendingPathComponent("web").path, "--host-dir", resources.appendingPathComponent("host").path, "--static", "--cache", "0", "--listen", "127.0.0.1:\(p.1)", "--api", String(r.1)], resources: resources)
            try await waitForHTTP(URL(string: "http://127.0.0.1:\(r.1)/health")!)
            try await waitForHTTP(URL(string: "http://127.0.0.1:\(p.1)/worlds/everglade")!)
            try Data((url.absoluteString + "\n").utf8).write(to: dataURL.appendingPathComponent("local-url.txt"), options: .atomic)
            notice.stringValue = "Loading Everglade…"
            refreshSetup(verify: false)
            if let metalWorld {
                metalLocalOrigin = "ws://127.0.0.1:\(r.1)"
                try metalWorld.open(resources: resources, storage: dataURL, page: url, local: metalLocalOrigin, muted: smoke || CommandLine.arguments.contains("--mute-audio"))
                web.load(URLRequest(url: URL(string: "http://127.0.0.1:\(p.1)/metal-voice.html")!))
            } else { web.load(URLRequest(url: url)) }
            if smoke && !nativeWorld {
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
            updateWorldVisibility()
            warmVoiceIfNeeded()
            if setupPane.isHidden { window.makeFirstResponder(worldView) }
            try await startSceneControls()
            if let i = CommandLine.arguments.firstIndex(of:"--web-preview-plan"),i+1<CommandLine.arguments.count {
                try await measureWebPreview(URL(fileURLWithPath:CommandLine.arguments[i+1])); return
            }
            if let i = CommandLine.arguments.firstIndex(of:"--metal-measure-plan"),i+1<CommandLine.arguments.count {
                try await measureMetal(URL(fileURLWithPath:CommandLine.arguments[i+1])); return
            }
            if let i = CommandLine.arguments.firstIndex(of: "--metal-observe"), i+1<CommandLine.arguments.count {
                let path = URL(fileURLWithPath: CommandLine.arguments[i+1])
                Task { while !stopping {
                    if let h = metalWorld?.host {
                        let state: [String: Any] = ["frames":mf_info(h,0),"completed":mf_info(h,10),"pending":mf_info(h,2),"draws":mf_info(h,1),"ledgerLive":mf_info(h,3),"ledgerWords":mf_info(h,4),"gpuNS":mf_info(h,11),"net":mf_info(h,7),"seat":mf_info(h,8),"paused":metalWorld?.paused ?? false,"setup":!setupPane.isHidden,"width":worldView.bounds.width,"height":worldView.bounds.height,"scale":window.backingScaleFactor,"x":mf_probe(h,0),"z":mf_probe(h,2),"messages":metalWorld?.network?.delivered ?? 0,"miniaturized":window.isMiniaturized,"visible":window.isVisible,"key":window.isKeyWindow,"captured":metalWorld?.captured ?? false,"audio":metalWorld?.audio?.source.observations() ?? [:]]
                        if let data = try? JSONSerialization.data(withJSONObject: state, options: [.sortedKeys]) { try? data.write(to: path, options: .atomic) }
                    }
                    try? await Task.sleep(for: .milliseconds(250))
                } }
            }
            if setupProfileURL != nil { await inventoryTask?.value; try await setupProfileCheck(); return }
            if offlineReportURL != nil {await inventoryTask?.value;try await offlineCheck();return}
            if let frameProfileURL { Task { try? await captureFrameProfile(frameProfileURL) } }
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
            if hotbarSmoke {
                await inventoryTask?.value
                try await hotbarSmokeCheck()
            } else if voiceWarmSmoke {
                await inventoryTask?.value
                try await voiceWarmSmokeCheck()
            } else if audioSmoke {
                try await audioSmokeCheck()
            } else if microphoneReport != nil {
                await inventoryTask?.value
                try await microphoneCheck()
            } else if npcSmoke {
                await inventoryTask?.value
                try await npcSmokeCheck(resources)
            } else if sceneSmoke {
                await inventoryTask?.value
                try await sceneSmokeCheck(resources)
            } else if inputSmoke {
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

    // SDK effects only: observe the running Coax player's position and publish
    // the local service address for the prebuilt Coax scene tool.
    func startSceneControls() async throws {
        let base = dataURL.deletingLastPathComponent()
        let session = base.appendingPathComponent("scene-session.json")
        let instructions = base.appendingPathComponent("scene-coder.txt")
        var bytes = [UInt8](repeating: 0, count: 8192)
        let n = fgc_scene_instructions(&bytes, Int64(bytes.count))
        guard n > 0 else { throw ShellError.failed("Scene tool context is unavailable.") }
        try Data(bytes.prefix(Int(n))).write(to: instructions, options: .atomic)
        try fm.setAttributes([.posixPermissions: 0o600], ofItemAtPath: instructions.path)
        chatPane.sceneSessionURL = session; chatPane.sceneContextURL = instructions
        try await updateSceneSession()
        sceneContextTask = Task {
            while !Task.isCancelled && !stopping {
                try? await Task.sleep(for: .milliseconds(500))
                if !Task.isCancelled && !stopping { try? await updateSceneSession() }
            }
        }
    }
    func updateSceneSession() async throws {
        guard !stopping, sceneReady, let session = chatPane.sceneSessionURL, let pageURL else { return }
        let value: String
        if let h = metalWorld?.host { value = "{\"x_cm\":\(Int((Double(mf_probe(h,0))/10).rounded())),\"z_cm\":\(Int((Double(mf_probe(h,2))/10).rounded()))}" }
        else { value = try await web.evaluateJavaScript("JSON.stringify({x_cm:Math.round(coaxial.fieldProbe(0)/10),z_cm:Math.round(coaxial.fieldProbe(2)/10)})") as? String ?? "{}" }
        var data = try JSONSerialization.jsonObject(with: Data(value.utf8)) as? [String: Any] ?? [:]
        data["schema"] = "aethyrforge.scene-session.v1"
        data["api"] = "http://127.0.0.1:\(pageURL.port!)"
        data["pid"] = ProcessInfo.processInfo.processIdentifier
        var stamp = timespec(); clock_gettime(CLOCK_MONOTONIC, &stamp)
        data["observed_ms"] = Int64(stamp.tv_sec) * 1000 + Int64(stamp.tv_nsec) / 1_000_000
        guard !stopping else { return }
        try JSONSerialization.data(withJSONObject: data, options: [.sortedKeys, .withoutEscapingSlashes]).write(to: session, options: .atomic)
        try fm.setAttributes([.posixPermissions: 0o600], ofItemAtPath: session.path)
    }

    func sceneSmokeCheck(_ resources: URL) async throws {
        guard let report, let session = chatPane.sceneSessionURL else { throw ShellError.failed("Scene fixture has no local tool session.") }
        enterWorld()
        let tool = resources.appendingPathComponent("aethyrforge-scene")
        func command(_ json: String? = nil) async throws -> [String: Any] {
            let args = json.map { ["apply", $0] } ?? ["show"]
            let data = try await ForgeCLI.run(tool, args + ["--session", session.path], timeout: 30)
            let state = try JSONSerialization.jsonObject(with: data) as? [String: Any] ?? [:]
            guard state["ok"] as? Bool == true else { throw ShellError.failed("Scene tool: \(state["error"] ?? "unknown failure")") }
            return state
        }
        func objects(_ state: [String: Any]) -> [[String: Any]] { state["objects"] as? [[String: Any]] ?? [] }
        let original = try await command()
        if CommandLine.arguments.contains("--scene-reopen") {
            let prior = try JSONSerialization.jsonObject(with: Data(contentsOf: report.deletingLastPathComponent().appendingPathComponent("result.json"))) as? [String: Any] ?? [:]
            guard objects(original).count == prior["totalObjects"] as? Int else { throw ShellError.failed("Scene objects did not survive reopening.") }
            if let middle = prior["resizedIndex"] as? Int {
                guard objects(original).first(where: { $0["index"] as? Int == middle })?["size_percent"] as? Int == 300 else { throw ShellError.failed("Individual crate resize did not survive reopening.") }
            }
            try JSONSerialization.data(withJSONObject: ["reopened": true, "totalObjects": objects(original).count, "resizePersisted": true]).write(to: report)
            stopChildren(); NSApp.terminate(nil); return
        }
        let firstCount = objects(original).count
        let getsBefore = try await web.evaluateJavaScript("__forgeRequests.filter(r=>r.method==='GET').length") as? Int ?? 0
        let row = try await command("{\"op\":\"place\",\"item\":\"crate\",\"count\":3,\"x_cm\":600,\"z_cm\":-1200}")
        let stack = try await command("{\"op\":\"place\",\"item\":\"crate\",\"count\":5,\"arrangement\":\"stack\",\"x_cm\":1200,\"z_cm\":-1200}")
        guard let rowGroup = row["group"] as? Int, let stackGroup = stack["group"] as? Int else { throw ShellError.failed("Scene tool returned no groups.") }
        var state = try await command()
        let pile = objects(state).filter { $0["group"] as? Int == stackGroup }
        guard objects(state).count == firstCount + 8, pile.count == 5,
              pile.compactMap({ $0["lift_cm"] as? Int }).sorted() == [0,93,186,279,372] else { throw ShellError.failed("Programmatic row/stack did not produce real scene objects.") }
        guard let rowMiddle = objects(state).filter({ $0["group"] as? Int == rowGroup }).sorted(by: { ($0["x_cm"] as? Int ?? 0) < ($1["x_cm"] as? Int ?? 0) }).dropFirst().first?["index"] as? Int,
              let resizeVersion = state["version"] as? Int else { throw ShellError.failed("Scene row had no middle object/version.") }
        _ = try await command("{\"op\":\"resize\",\"index\":\(rowMiddle),\"version\":\(resizeVersion),\"scale_percent\":300}")
        state = try await command()
        guard objects(state).first(where: { $0["index"] as? Int == rowMiddle })?["size_percent"] as? Int == 300 else { throw ShellError.failed("Programmatic individual resize failed.") }
        var staleRefused = false
        do { _ = try await command("{\"op\":\"resize\",\"index\":\(rowMiddle),\"version\":\(resizeVersion),\"size_percent\":100}") } catch { staleRefused = true }
        guard staleRefused else { throw ShellError.failed("An old version resized a selected object.") }
        _ = try await command("{\"op\":\"move\",\"group\":\(rowGroup),\"dz_cm\":400}")
        _ = try await command("{\"op\":\"remove\",\"group\":\(stackGroup)}")
        let beforeBad = try await command()
        var refused = false
        do { _ = try await command("{\"op\":\"place\",\"item\":\"crate\",\"x_cm\":0,\"z_cm\":0}") } catch { refused = true }
        state = try await command()
        guard refused, state["version"] as? Int == beforeBad["version"] as? Int else { throw ShellError.failed("Blocked placement changed the scene.") }
        let root = report.deletingLastPathComponent()
        chatPane.home = root; chatPane.cwd = root.appendingPathComponent("workspace"); chatPane.cliOverride = resources.appendingPathComponent("openagents")
        try fm.createDirectory(at: chatPane.cwd, withIntermediateDirectories: true)
        try fm.createDirectory(at: root.appendingPathComponent(".openagents/aethyrforge"), withIntermediateDirectories: true)
        let router = root.appendingPathComponent(".openagents/coder/router.json")
        try fm.createDirectory(at: router.deletingLastPathComponent(), withIntermediateDirectories: true)
        if !fm.fileExists(atPath: router.path) { try fm.copyItem(at: fm.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/coder/router.json"), to: router) }
        var sessions: [String] = []
        var resizedIndex = -1
        for (turn, prompt) in ["Add three crates to the scene.", "Make the middle crate of the three you just added 3x larger. Keep the other two crates unchanged.", "Add a stack of five crates to the scene."].enumerated() {
            let before = try await command()
            chatPane.startTurn(prompt, voice: true)
            let end = Date().addingTimeInterval(120)
            while chatPane.busy && Date() < end {
                if chatPane.pending { throw ShellError.failed("Scene Coder unexpectedly requested tool approval.") }
                try await Task.sleep(for: .milliseconds(100))
            }
            guard !chatPane.busy, chatPane.lastEnding.hasPrefix("finished") else { throw ShellError.failed("Scene Coder failed: \(chatPane.transcript.string.suffix(2000))") }
            state = try await command(); let added = objects(state).count - objects(before).count
            guard added == [3,0,5][turn] else { throw ShellError.failed("Coder said it edited the scene but added \(added) objects: \(chatPane.transcript.string.suffix(2000))") }
            if turn == 1 {
                let beforeObjects = objects(before)
                let row = Array(beforeObjects.suffix(3)).sorted { ($0["x_cm"] as? Int ?? 0) < ($1["x_cm"] as? Int ?? 0) }
                guard row.count == 3, let middle = row[1]["index"] as? Int else { throw ShellError.failed("Natural-language row has no middle crate.") }
                resizedIndex = middle
                for (old, new) in zip(beforeObjects, objects(state)) {
                    var intended = old
                    if old["index"] as? Int == middle { intended["size_percent"] = 300 }
                    guard NSDictionary(dictionary: intended).isEqual(to: new) else { throw ShellError.failed("Coder resized or rebuilt the wrong objects: \(old) -> \(new)") }
                }
            }
            sessions.append(chatPane.session)
        }
        let latest = objects(state).suffix(5)
        guard latest.compactMap({ $0["lift_cm"] as? Int }).sorted() == [0,93,186,279,372], Set(latest.compactMap({ $0["group"] as? Int })).count == 1 else { throw ShellError.failed("Natural-language stack was not stacked.") }
        try await Task.sleep(for: .seconds(3))
        let getsAfter = try await web.evaluateJavaScript("__forgeRequests.filter(r=>r.method==='GET' && r.status===200).length") as? Int ?? 0
        guard getsAfter > getsBefore else { throw ShellError.failed("The page did not fetch the changed world.") }
        try await captureCanvas(report)
        let result: [String: Any] = ["ok": true, "programmaticRow": true, "programmaticStack": true, "groupMoveRemove": true,
            "blockedRefused": refused, "staleResizeRefused": staleRefused, "resizedIndex": resizedIndex, "middleSizePercent": 300, "neighboursUnchanged": true, "naturalLanguageSessions": sessions, "pageFetchedChanges": true,
            "totalObjects": objects(state).count, "version": state["version"] ?? 0, "engine": chatPane.selectedEngine]
        try JSONSerialization.data(withJSONObject: result, options: [.prettyPrinted, .sortedKeys]).write(to: report)
        stopChildren(); NSApp.terminate(nil)
    }

    func npcSmokeCheck(_ resources: URL) async throws {
        guard let report, let session = chatPane.sceneSessionURL else { throw ShellError.failed("NPC check has no scene session.") }
        enterWorld()
        let tool = resources.appendingPathComponent("aethyrforge-scene")
        func command(_ json: String? = nil) async throws -> [String: Any] {
            let args = json.map { ["npc", $0] } ?? ["npcs"]
            let data = try await ForgeCLI.run(tool, args + ["--session", session.path], timeout: 30)
            let state = try JSONSerialization.jsonObject(with: data) as? [String: Any] ?? [:]
            guard state["ok"] as? Bool == true else { throw ShellError.failed("NPC tool: \(state["error"] ?? "unknown failure")") }
            return state
        }
        func people(_ state: [String: Any]) -> [[String: Any]] { state["npcs"] as? [[String: Any]] ?? [] }
        func probe(_ field: Int) async throws -> Int { try await web.evaluateJavaScript("coaxial.netProbe(\(field))") as? Int ?? -1 }
        if CommandLine.arguments.contains("--npc-reopen") {
            let state = try await command()
            guard people(state).count == 8 else { throw ShellError.failed("Authored NPCs did not survive reopening.") }
            let raidData = try await ForgeCLI.run(tool, ["raid", "--session", session.path], timeout: 30)
            let raid = try JSONSerialization.jsonObject(with: raidData) as? [String: Any] ?? [:]
            guard raid["active"] as? Bool == false, raid["cult_alive"] as? Int == 0 else { throw ShellError.failed("A transient raid replayed on reopening.") }
            try JSONSerialization.data(withJSONObject: ["ok": true, "reopened": true, "npcs": 8]).write(to: report)
            stopChildren(); NSApp.terminate(nil); return
        }
        _ = try await command("{\"op\":\"spawn\",\"kind\":\"cultist\",\"count\":2,\"x_cm\":600,\"z_cm\":-1200}")
        _ = try await command("{\"op\":\"spawn\",\"kind\":\"villager\",\"x_cm\":600,\"z_cm\":-1000}")
        let beforeBad = try await command()
        var refused = false
        do { _ = try await command("{\"op\":\"spawn\",\"kind\":\"cultist\",\"x_cm\":0,\"z_cm\":0}") } catch { refused = true }
        let afterBad = try await command()
        guard refused, beforeBad["version"] as? Int == afterBad["version"] as? Int else { throw ShellError.failed("Blocked NPC spawn changed the document.") }
        var crateRefused = false
        do { _ = try await command("{\"op\":\"spawn\",\"kind\":\"villager\",\"x_cm\":0,\"z_cm\":-8251}") } catch { crateRefused = true }
        guard crateRefused else { throw ShellError.failed("NPC spawn ignored the existing crate footprint.") }
        let root = report.deletingLastPathComponent()
        chatPane.home = root; chatPane.cwd = root.appendingPathComponent("workspace"); chatPane.cliOverride = resources.appendingPathComponent("openagents")
        try fm.createDirectory(at: chatPane.cwd, withIntermediateDirectories: true)
        try fm.createDirectory(at: root.appendingPathComponent(".openagents/aethyrforge"), withIntermediateDirectories: true)
        let router = root.appendingPathComponent(".openagents/coder/router.json")
        try fm.createDirectory(at: router.deletingLastPathComponent(), withIntermediateDirectories: true)
        if !fm.fileExists(atPath: router.path) { try fm.copyItem(at: fm.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/coder/router.json"), to: router) }
        var sessions: [String] = []
        func turn(_ prompt: String) async throws {
            chatPane.startTurn(prompt, voice: true)
            let end = Date().addingTimeInterval(120)
            while chatPane.busy && Date() < end {
                if chatPane.pending { throw ShellError.failed("NPC Coder requested an approval.") }
                try await Task.sleep(for: .milliseconds(100))
            }
            guard !chatPane.busy, chatPane.lastEnding.hasPrefix("finished") else { throw ShellError.failed("NPC Coder failed: \(chatPane.transcript.string.suffix(1600))") }
            sessions.append(chatPane.session)
        }
        try await turn("Have the cultists in group 1 attack villager NPC 3.")
        var state = try await command()
        guard people(state).prefix(2).allSatisfy({ $0["action"] as? String == "attack" && $0["target_id"] as? Int == 3 }) else { throw ShellError.failed("Coder did not set the group attack order.") }
        let hurtEnd = Date().addingTimeInterval(45)
        while Date() < hurtEnd {
            state = try await command()
            if let victim = people(state).first(where: { $0["id"] as? Int == 3 }), (victim["hp"] as? Int ?? 4) < 4 { break }
            try await Task.sleep(for: .milliseconds(300))
        }
        guard let victim = people(state).first(where: { $0["id"] as? Int == 3 }), (victim["hp"] as? Int ?? 4) < 4 else { throw ShellError.failed("The directed attack did not deal real damage.") }
        try await turn("Add three villagers nearby.")
        state = try await command()
        guard people(state).count == 6 else { throw ShellError.failed("Coder did not spawn exactly three villagers.") }
        let breaksBefore = try await probe(26)
        try await turn("Stop the cultists in group 1. Add two new cultists centred at x=6 metres, z=-70 metres, then order that new group to attack the stables.")
        state = try await command()
        guard people(state).count == 8, people(state).suffix(2).allSatisfy({ $0["action"] as? String == "attack" && $0["target_id"] as? Int == 9 }) else { throw ShellError.failed("Coder did not direct the new cultists at the actual stables.") }
        let breakEnd = Date().addingTimeInterval(90)
        while Date() < breakEnd {
            if try await probe(26) > breaksBefore { break }
            try await Task.sleep(for: .milliseconds(300))
        }
        let breaksAfter = try await probe(26)
        guard breaksAfter > breaksBefore else { throw ShellError.failed("The building attack did not break real replicated pieces.") }
        _ = try await command("{\"op\":\"order\",\"group\":4,\"action\":\"stop\"}")
        try await Task.sleep(for: .seconds(1))
        let visible = try await probe(16)
        guard visible >= 7 else { throw ShellError.failed("Spawned NPCs did not reach the rendered page: \(visible).") }
        func raid(_ json: String? = nil) async throws -> [String: Any] {
            let args = json.map { ["raid", $0] } ?? ["raid"]
            let data = try await ForgeCLI.run(tool, args + ["--session", session.path], timeout: 30)
            let state = try JSONSerialization.jsonObject(with: data) as? [String: Any] ?? [:]
            guard state["ok"] as? Bool == true else { throw ShellError.failed("Raid tool: \(state["error"] ?? "unknown failure")") }
            return state
        }
        try await turn("Trigger the planned cultist raid world event now.")
        var event = try await raid()
        guard event["active"] as? Bool == true, event["cult_target"] as? Int == 30,
              event["watch_target"] as? Int == 24, event["wards"] as? Int == 3 else { throw ShellError.failed("Coder did not start the planned siege with its watch and wards: \(event)") }
        try await turn("Double the cultist army in the event. Keep the town watch the same size.")
        event = try await raid()
        guard event["cult_target"] as? Int == 60, (event["sent"] as? Int ?? 0) >= 60,
              event["watch_target"] as? Int == 24 else { throw ShellError.failed("Coder did not double the real cult army: \(event)") }
        let armyVisible = try await probe(16)
        guard armyVisible >= 60 else { throw ShellError.failed("The event army did not reach the rendered page: \(armyVisible)") }
        let priorVersion = event["version"] as? Int
        var overflowRefused = false
        do { _ = try await raid("{\"op\":\"double\"}") } catch { overflowRefused = true }
        event = try await raid()
        guard overflowRefused, event["version"] as? Int == priorVersion, event["cult_target"] as? Int == 60 else { throw ShellError.failed("Overflow changed the event army.") }
        try await captureCanvas(report)
        try await turn("Set the event cultist army to 10 fighters, then add another wave of five cult fighters.")
        event = try await raid()
        guard event["cult_target"] as? Int == 15 else { throw ShellError.failed("Coder did not add the requested event wave: \(event)") }
        _ = try await raid("{\"op\":\"stop\"}")
        event = try await raid();state = try await command()
        guard event["active"] as? Bool == false, event["cult_alive"] as? Int == 0,
              event["watch_alive"] as? Int == 0, people(state).count == 8 else { throw ShellError.failed("Stopping the event failed or removed authored NPCs.") }
        let result: [String: Any] = ["ok": true, "npcs": 8, "routedSessions": sessions, "npcDamage": true, "buildingBreaks": breaksAfter - breaksBefore, "visibleCreatures": visible, "blockedSpawnRefused": refused, "crateSpawnRefused": crateRefused, "eventDoubledTo": 60, "eventVisibleCreatures": armyVisible, "eventWaveTarget": 15, "eventOverflowRefused": overflowRefused, "eventStopped": true]
        try JSONSerialization.data(withJSONObject: result, options: [.prettyPrinted, .sortedKeys]).write(to: report)
        stopChildren(); NSApp.terminate(nil)
    }

    func waitForScene() async throws {
        if let metalWorld {
            let end = ProcessInfo.processInfo.systemUptime + 60
            while !stopping && ProcessInfo.processInfo.systemUptime < end {
                if metalWorld.completed { return }
                if let h = metalWorld.host, let error = mf_error(h) { throw ShellError.failed(String(cString: error)) }
                try await Task.sleep(for: .milliseconds(20))
            }
            throw ShellError.failed("Metal did not complete its first real frame.")
        }
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
    // Raw browser and platform observations; the Coax profiler owns statistics.
    func captureFrameProfile(_ url: URL) async throws {
        if benchmarkReportURL != nil {do{try await renderBenchmark(url)}catch{fail(error.localizedDescription)};return}
        try await Task.sleep(for: .seconds(3))
        _ = try await web.evaluateJavaScript("coaxial.takeFrameTrace();true")
        var windows: [[String: Any]] = []
        let walking = CommandLine.arguments.contains("--frame-profile-walk")
        func walkKey(_ key: String, _ down: Bool) async throws {
            _ = try await web.evaluateJavaScript("dispatchEvent(new KeyboardEvent('\(down ? "keydown" : "keyup")',{key:'\(key)',code:'Key\(key.uppercased())',bubbles:true}));true")
        }
        for _ in 0..<6 {
            if walking { try await walkKey("w", true) }
            try await Task.sleep(for: .milliseconds(2500))
            if walking { try await walkKey("w", false); try await walkKey("s", true) }
            try await Task.sleep(for: .milliseconds(2500))
            if walking { try await walkKey("s", false) }
            guard !stopping else { return }
            let raw = try await web.evaluateJavaScript("JSON.stringify({samples:coaxial.takeFrameTrace(),quality:Array.from({length:9},(_,i)=>coaxial.quality(i)),queue:coaxial.frameQueue(),hidden:document.hidden,focused:document.hasFocus()})") as? String ?? "{}"
            var snapshot = try JSONSerialization.jsonObject(with: Data(raw.utf8)) as? [String: Any] ?? [:]
            snapshot["visible"] = window.isVisible && window.occlusionState.contains(.visible)
            snapshot["thermal"] = ProcessInfo.processInfo.thermalState.rawValue
            snapshot["lowPower"] = ProcessInfo.processInfo.isLowPowerModeEnabled
            snapshot["displayFPS"] = window.screen?.maximumFramesPerSecond ?? 0
            snapshot["highRefreshEnabled"] = highRefreshEnabled
            snapshot["active"] = NSApp.isActive
            snapshot["walking"] = walking
            windows.append(snapshot)
            try JSONSerialization.data(withJSONObject: ["schema":"aethyrforge.frame-profile.v1","windows":windows], options: [.sortedKeys]).write(to: url, options: .atomic)
        }
    }

    // Native Web Audio regression check: observe the real realm's mix before
    // the test-only silent output, and inspect normal configuration separately.
    func audioSmokeCheck() async throws {
        guard let report else { throw ShellError.failed("--audio-smoke needs a report path.") }
        let config = WKWebViewConfiguration()
        configureAudioOutput(config, muted: false)
        let normal = WKWebView(frame: .zero, configuration: config)
        normal.loadHTMLString("<!doctype html><title>Audio configuration check</title>", baseURL: nil)
        while normal.isLoading { try await Task.sleep(for: .milliseconds(20)) }
        guard try await normal.evaluateJavaScript("typeof globalThis.coaxialAudioTap === 'undefined'") as? Bool == true else {
            throw ShellError.failed("Normal desktop configuration still overrides Everglade's speaker output.")
        }
        normal.stopLoading()
        _ = try await web.evaluateJavaScript("""
        globalThis.__forgeAudioCheck = null;
        (async()=>{
          const silent=globalThis.coaxialAudioTap;
          let meter,sink;
          globalThis.coaxialAudioTap=c=>{sink=silent(c);meter=c.createAnalyser();meter.fftSize=2048;meter.connect(sink);return meter;};
          const ctx=coaxial.audio.start();await ctx.resume();
          const samples=new Float32Array(meter.fftSize);let peak=0,cues=0;
          const end=performance.now()+5000;
          while(performance.now()<end){
            if(!cues)cues=coaxial.audio.probe(-1,0,1,0);
            meter.getFloatTimeDomainData(samples);for(const s of samples)peak=Math.max(peak,Math.abs(s));
            await new Promise(r=>setTimeout(r,40));
          }
          __forgeAudioCheck={normalOutput:true,context:ctx.state,peak,cues,testOutputGain:sink.gain.value,beds:coaxial.audio.beds(),microphoneRequested:false};
        })().catch(e=>{__forgeAudioCheck={error:String(e)}});true
        """)
        let deadline = Date().addingTimeInterval(15)
        while Date() < deadline {
            if try await web.evaluateJavaScript("!!globalThis.__forgeAudioCheck") as? Bool == true { break }
            try await Task.sleep(for: .milliseconds(100))
        }
        let raw = try await web.evaluateJavaScript("JSON.stringify(__forgeAudioCheck)") as? String ?? "null"
        guard let state = try JSONSerialization.jsonObject(with: Data(raw.utf8)) as? [String:Any],
              state["context"] as? String == "running", (state["peak"] as? Double ?? 0) > 0.00001,
              (state["cues"] as? Int ?? 0) > 0, state["testOutputGain"] as? Double == 0 else {
            throw ShellError.failed("Everglade did not produce live sound through its silent test output: \(raw)")
        }
        try Data(raw.utf8).write(to: report, options: .atomic)
        stopChildren(); NSApp.terminate(nil)
    }

    func configureAudioOutput(_ config: WKWebViewConfiguration, muted: Bool) {
        // Test output stays connected so AudioWorklets run. Ordinary windows
        // use the realm's normal AudioContext destination and SOUND controls.
        guard muted else { return }
        config.userContentController.addUserScript(WKUserScript(source: "globalThis.coaxialAudioTap = c => {const g=c.createGain();g.gain.value=0;g.connect(c.destination);return g;};", injectionTime: .atDocumentStart, forMainFrameOnly: true))
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
            _ = try await web.evaluateJavaScript("dispatchEvent(new KeyboardEvent('keydown',{key:'b',code:'KeyB',bubbles:true})); dispatchEvent(new KeyboardEvent('keyup',{key:'b',code:'KeyB',bubbles:true})); true")
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
        if let report=setupProfileURL {try? Data(message.utf8).write(to:report.appendingPathExtension("error"));fputs("Aethyrforge setup profile: \(message)\n",stderr);exit(1)}
        if let report=offlineReportURL {try? Data(message.utf8).write(to:report.appendingPathExtension("error"));fputs("Aethyrforge offline check: \(message)\n",stderr);exit(1)}
        if let report=benchmarkReportURL {try? Data(message.utf8).write(to:report.appendingPathExtension("error"));fputs("Aethyrforge benchmark: \(message)\n",stderr);exit(1)}
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

    var settingsURL: URL {
        if let isolated = (smoke ? report : microphoneReport)?.deletingLastPathComponent() { return isolated.appendingPathComponent("setup.json") }
        return fm.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/aethyrforge/setup.json")
    }
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
        refreshMicrophoneSetup()
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
            warmVoiceIfNeeded()
        }
    }
    func enterWorld() {
        guard fgs_can_enter(sceneReady) else { return }
        var bytes: [UInt8] = []
        for i in 0..<512 { let c = fgs_settings_byte(Int64(i)); if c < 0 { break }; bytes.append(UInt8(c)) }
        do {
            let saved = Data(bytes)
            try saved.write(to: settingsURL, options: .atomic)
            try acceptSetup(saved, kind: 3)
            setupPane.isHidden = true; window.makeFirstResponder(worldView)
            setupPane.statusMode = false; updateWorldVisibility()
        } catch { setupPane.errorMessage = "Could not save setup: \(error.localizedDescription)"; setupPane.update() }
    }
    @objc func showSetup() { cancelVoice(); chatPane.closeComposer(); setupPane.statusMode = false; setupPane.isHidden = false; setupPane.sceneReady = sceneReady; updateWorldVisibility(); setupPane.update(); window.makeFirstResponder(setupPane); refreshMicrophoneSetup() }
    @objc func showStatus() { showSetup(); setupPane.statusMode = true; setupPane.update() }
    func updateWorldVisibility() {
        guard web != nil, setupPane != nil, window != nil else { return }
        let paused = forge_world_paused(sceneReady,!setupPane.isHidden,smoke || (window.isVisible && !NSApp.isHidden),!smoke && window.isMiniaturized,!smoke && !window.occlusionState.contains(.visible))
        setupPane.worldPaused = paused
        guard worldPauseWanted != paused else { return }
        worldPauseWanted = paused; setupPane.update()
        if let metalWorld { metalWorld.setPaused(paused) }
        else { web.evaluateJavaScript("globalThis.coaxial?.setWorldPaused(\(paused)); true", completionHandler: nil) }
    }
    @objc func focusChat() { guard setupPane.isHidden else { return }; chatPane.openComposer() }
    func routeCoderKey(_ event: NSEvent) -> Bool {
        guard event.type == .keyDown || event.type == .keyUp else { return false }
        let down = event.type == .keyDown
        let modified = !event.modifierFlags.intersection([.command, .control, .option]).isEmpty
        let ready = sceneReady && setupPane.isHidden && NSApp.modalWindow == nil && window.attachedSheet == nil
        let realmFocused = (window.firstResponder as? NSView)?.isDescendant(of: worldView) == true
        let route = fgc_enter_route(ready, chatPane.composerOpen && !realmFocused, Int64(event.keyCode), down, event.isARepeat, modified, coderHeldKeys.contains(event.keyCode))
        if route != 0 {
            if down { coderHeldKeys.insert(event.keyCode) } else { coderHeldKeys.remove(event.keyCode) }
        }
        if route == 2 { focusChat() }
        return route == 1 || route == 2
    }

    // Real AppKit key dispatch over the packaged realm, with no visible window.
    // The staged events separately exercise presentation while the CLI waits.
    // Hidden WKWebView acceptance uses the same keyboard/pointer listeners and
    // room kit as normal play, with its own world and local storage.
    func hotbarSmokeCheck() async throws {
        guard let report else { throw ShellError.failed("--hotbar-smoke needs a report path.") }
        enterWorld()
        var results: [[String:Any]] = []
        for stage in 0..<2 {
            let result = try await web.callAsyncJavaScript("""
            const sleep=ms=>new Promise(r=>setTimeout(r,ms));
            const wait=async f=>{const end=performance.now()+10000;while(!f()&&performance.now()<end)await sleep(20);if(!f())throw Error('Hotbar did not reach the expected state');};
            const key=(type,key,shiftKey=false)=>dispatchEvent(new KeyboardEvent(type,{key,shiftKey,bubbles:true}));
            const press=k=>{key('keydown',k);key('keyup',k);};
            const p=(b,f=0)=>coaxial.hotbarProbe(b,f);
            const box=b=>[p(b,1),p(b,2),p(b,3),p(b,4)];
            const pointer=(type,b)=>{const v=box(b),c=document.querySelector('canvas'),scale=c.width/c.clientWidth;dispatchEvent(new PointerEvent(type,{pointerType:'mouse',pointerId:7,clientX:(v[0]+v[2])/2/scale,clientY:(v[1]+v[3])/2/scale,button:0,buttons:type==='pointerup'?0:1,bubbles:true}));};
            try {
              await wait(()=>coaxial.sceneReady&&p(21)>=0&&p(22)>=0);
              if(p(21,6)!==0)throw Error('The editor opened at launch');
              await wait(()=>p(21,3)>p(21,1)&&p(22,3)>p(22,1));
              press('b');await wait(()=>p(21,6)===1);press('b');await wait(()=>p(21,6)===0);
              const before={a:p(21),b:p(22),boxes:[box(21),box(22)]};
              if(stage===0){
                if(before.a!==0||before.b!==1)throw Error('Expected a fresh room kit');
                key('keydown','Shift',true);pointer('pointerdown',21);pointer('pointermove',22);pointer('pointerup',22);key('keyup','Shift');
              }
              await wait(()=>p(21)===1&&p(22)===0&&box(21)[0]>box(22)[0]);
              if(p(21,5)!==-1)throw Error('Dragging cast a spell');
              // The actual number keys must select the newly assigned spells.
              press('1');const on1=p(21,5);press('1');press('2');const on2=p(21,5);press('2');
              if(on1!==22||on2!==21)throw Error('Number keys did not follow the swapped icons: '+on1+','+on2);
              key('keydown','Shift',true);pointer('pointerdown',21);pointer('pointermove',22);pointer('pointercancel',22);key('keyup','Shift');
              if(p(21)!==1||p(22)!==0)throw Error('Cancelled drag changed assignment');
              await wait(()=>!!localStorage.getItem('coaxial.everglade.bar'));
              const saved=localStorage.getItem('coaxial.everglade.bar');
              const places=saved.split('places=')[1].trim().split(',').map(Number);
              if(places.length!==47||new Set(places).size!==47||places.some(v=>v<0))throw Error('Reduced kit corrupted the saved arrangement');
              return {ok:true,stage,before,after:{a:p(21),b:p(22),boxes:[box(21),box(22)]},on1,on2,cancelled:true,saved};
            } catch(e) {return {ok:false,stage,error:String(e)};}
            """, arguments: ["stage":stage], in: nil, contentWorld: .page) as? [String:Any] ?? [:]
            guard result["ok"] as? Bool == true else { throw ShellError.failed("Hotbar input regression: \(result)") }
            results.append(result)
            if stage == 0 {
                reloadWorld()
                try await Task.sleep(for: .milliseconds(300))
                try await waitForScene()
            }
        }
        try await captureCanvas(report)
        try JSONSerialization.data(withJSONObject:["ok":true,"webkit":true,"editorClosedOnLaunchAndReload":true,"editorOpensOnlyOnRequest":true,"shiftDragAndNumberKeys":true,"reloadPreservesAssignment":true,"stages":results],options:[.prettyPrinted,.sortedKeys]).write(to:report,options:.atomic)
        stopChildren();NSApp.terminate(nil)
    }

    func inputSmokeCheck() async throws {
        guard let report else { throw ShellError.failed("--input-smoke needs a report path.") }
        enterWorld()
        func require(_ ok: Bool, _ message: String) throws { if !ok { throw ShellError.failed(message) } }
        func dropdowns(_ view: NSView) -> Int { (view is NSPopUpButton || view is NSComboBox ? 1 : 0) + view.subviews.reduce(0) { $0 + dropdowns($1) } }
        try require(dropdowns(chatPane) == 0 && dropdowns(setupPane) == 0, "Agent and model dropdowns must be replaced by automatic routing.")
        func key(_ code: UInt16, down: Bool = true, repeatKey: Bool = false, modifiers: NSEvent.ModifierFlags = []) {
            let text = code == 53 ? "\u{1b}" : "\r"
            let event = NSEvent.keyEvent(with: down ? .keyDown : .keyUp, location: .zero, modifierFlags: modifiers, timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window.windowNumber, context: nil, characters: text, charactersIgnoringModifiers: text, isARepeat: repeatKey, keyCode: code)!
            window.sendEvent(event)
        }
        try require(setupPane.isHidden && chatPane.presentation == 0 && chatPane.hud.isHidden && chatPane.inputScroll.isHidden && chatPane.transcriptScroll.isHidden && chatPane.messageHUD.isHidden, "Coder must be completely absent at world entry.")
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
            window.makeFirstResponder(worldView); key(code); key(code, down: false)
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
        try require(chatPane.presentation == 1 && !chatPane.messageHUD.isHidden && chatPane.hud.isHidden && chatPane.heading.isHidden && chatPane.status.isHidden && chatPane.inputScroll.isHidden && chatPane.messageText == "Active reply on the left\n\n", "An active reply must show only message text.")
        try require(chatPane.hitTest(NSPoint(x: 30, y: 500)) == nil, "Closed Coder messages must not block the realm.")
        chatPane.appendMessage("Aethyr draws this message and owns its five-second lifetime.\nCurrent folder: ~/.openagents/aethyrforge/workspace\n")
        let feedVertices = try chatPane.messageHUD.saveCheckFrame(report.deletingPathExtension().appendingPathExtension("png"))
        chatPane.receive("{\"kind\":\"ask\",\"text\":\"Review this request\"}")
        try require(!chatPane.composerOpen && chatPane.allow.isHidden && chatPane.messageText.contains("Press Enter to review"), "Pending tools must wait for review without opening controls.")
        key(36); key(36, down: false)
        try require(!chatPane.allow.isHidden && !chatPane.deny.isHidden, "Enter must reveal the pending tool's approval controls.")
        key(53); key(53, down: false)
        try require(chatPane.busy && chatPane.pending && !chatPane.composerOpen, "Escape must close without stopping the agent or answering a tool.")
        chatPane.busy = false; chatPane.pending = false; chatPane.finishMessages()
        try require(chatPane.presentation == 1 && !chatPane.messageHUD.isHidden, "Completed messages must remain visible while Coder is closed.")
        key(36); key(36, down: false)
        try require(chatPane.transcript.string.contains("Active reply on the left"), "Enter must restore conversation history.")
        chatPane.newChat(); chatPane.closeComposer()
        let root = report.deletingLastPathComponent()
        chatPane.home = root; chatPane.cliOverride = Bundle.main.resourceURL!.appendingPathComponent("openagents")
        chatPane.cwd = root.appendingPathComponent("workspace")
        try fm.createDirectory(at: chatPane.cwd, withIntermediateDirectories: true)
        try fm.createDirectory(at: root.appendingPathComponent(".openagents/aethyrforge"), withIntermediateDirectories: true)
        chatPane.engineOverride = "echo"
        let originalSceneContext = chatPane.sceneContextURL
        chatPane.sceneContextURL = nil // Echo exercises input/streaming, without slowly echoing the tool manual.
        defer { chatPane.sceneContextURL = originalSceneContext }
        key(36); key(36, down: false)
        chatPane.input.string = "FORGE_ENTER_CODER_OK"
        key(36); key(36, repeatKey: true); key(36, down: false)
        try require(chatPane.busy && !chatPane.composerOpen && chatPane.inputScroll.isHidden, "Send must collapse the composer and leave the run active.")
        let deadline = Date().addingTimeInterval(30)
        while chatPane.busy && Date() < deadline { try await Task.sleep(for: .milliseconds(50)) }
        try require(!chatPane.busy && chatPane.lastEnding.hasPrefix("finished") && chatPane.transcript.string.contains("FORGE_ENTER_CODER_OK") && !chatPane.session.isEmpty && chatPane.presentation == 1, "Native Enter did not finish its real Coder CLI round trip with the reply still visible.")
        let finishedAt = ProcessInfo.processInfo.systemUptime
        try await Task.sleep(for: .milliseconds(4500))
        try require(!chatPane.messageHUD.isHidden && chatPane.hud.isHidden && chatPane.inputScroll.isHidden && chatPane.hitTest(NSPoint(x: 30, y: 500)) == nil, "The finished reply disappeared early or opened controls.")
        let hideDeadline = ProcessInfo.processInfo.systemUptime + 2
        while !chatPane.messageHUD.isHidden && ProcessInfo.processInfo.systemUptime < hideDeadline { try await Task.sleep(for: .milliseconds(25)) }
        let lingered = ProcessInfo.processInfo.systemUptime - finishedAt
        try require(chatPane.presentation == 0 && chatPane.messageHUD.isHidden && lingered >= 4.9 && lingered < 6.5, "Completed reply did not hide after five seconds: \(lingered).")
        let realm = try await web.evaluateJavaScript("JSON.stringify({downs:__forgeEnterDown,ups:__forgeEnterUp,typing:!!document.querySelector('input[aria-label=\"Typed text\"]'),scene:coaxial.sceneReady})") as? String ?? "{}"
        let state = try JSONSerialization.jsonObject(with: Data(realm.utf8)) as? [String: Any] ?? [:]
        try require(state["downs"] as? Int == 0 && state["ups"] as? Int == 0 && state["typing"] as? Bool == false && state["scene"] as? Bool == true, "Native Enter reached the realm's region chat: \(realm)")
        let result: [String: Any] = ["ok": true, "realm": state, "nativeReturnAndKeypad": true, "heldEnter": true, "shiftEnter": true, "escapeKeepsDraftAndRun": true, "replyLingerSeconds": lingered, "feedComponent": "aethyr/ui/feed-draw.coax", "feedVertices": feedVertices, "realCoder": chatPane.session, "ending": chatPane.lastEnding]
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
            chatPane.engineOverride = engine == "auto" ? nil : engine
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
            if engine == "auto" {
                for id in [first, chatPane.session] {
                    let turn = try String(contentsOf: chatPane.sessions.appendingPathComponent(id + ".jsonl"), encoding: .utf8)
                    let records = turn.split(separator: "\n").compactMap { try? JSONSerialization.jsonObject(with: Data($0.utf8)) as? [String: Any] }
                    guard records.contains(where: { $0["kind"] as? String == "route" && $0["schema"] as? String == "coder.route.v2" && $0["ok"] as? Bool == true }) else { throw ShellError.failed("Automatic first/follow-up turn did not receive an Elephant decision.") }
                }
            }
            let followup = chatPane.session
            var approved = false
            var approvalRequests = 0
            var stopped = false
            if ProcessInfo.processInfo.environment["FORGE_CHAT_SMOKE_ACTIONS"] == "1" {
                chatPane.input.string = "Use a shell command to write exactly HUD_APPROVED to hud-approved.txt in the working directory. Do not write any other file."
                chatPane.submit(); let end = Date().addingTimeInterval(120)
                while chatPane.busy && Date() < end {
                    if chatPane.pending { approvalRequests += 1; chatPane.stopTurn(); break }
                    try await Task.sleep(for: .milliseconds(50))
                }
                approved = (try? String(contentsOf: chatPane.cwd.appendingPathComponent("hud-approved.txt"), encoding: .utf8))?.trimmingCharacters(in: .whitespacesAndNewlines) == "HUD_APPROVED"
                guard !chatPane.busy, approved, approvalRequests == 0 else { throw ShellError.failed("Coder unattended write flow failed: \(chatPane.transcript.string.suffix(1200))") }
                chatPane.input.string = "Run the shell command sleep 60, then reply DONE."
                chatPane.submit(); let stopEnd = Date().addingTimeInterval(30)
                while chatPane.busy && chatPane.session.isEmpty && Date() < stopEnd { try await Task.sleep(for: .milliseconds(50)) }
                chatPane.stopTurn()
                while chatPane.busy && Date() < stopEnd { try await Task.sleep(for: .milliseconds(50)) }
                stopped = !chatPane.busy && chatPane.lastEnding.hasPrefix("stopped")
                guard stopped else { throw ShellError.failed("Coder did not stop the active turn.") }
            }
            let result: [String: Any] = ["ok": true, "engine": engine, "actualEngine": chatPane.selectedEngine, "route": chatPane.router.stringValue, "first": first, "followup": followup, "ending": chatPane.lastEnding, "unattendedWrite": approved, "approvalRequests": approvalRequests, "stop": stopped, "hud": "aethyr/ui/feed-draw.coax", "cli": chatPane.cli.path]
            try JSONSerialization.data(withJSONObject: result, options: [.prettyPrinted, .sortedKeys]).write(to: report)
            print("Coder HUD smoke passed: \(engine)")
        } catch { if let report { try? Data(error.localizedDescription.utf8).write(to: report.appendingPathExtension("error")) }; print(error.localizedDescription) }
        chatPane.shutdown(); NSApp.terminate(nil)
    }

    // SDK acceptance observes real local WELCOME and worklet preparation while
    // prohibiting hardware microphone access. Silence then exercises three holds.
    func voiceWarmSmokeCheck() async throws {
        guard let report else { throw ShellError.failed("--voice-warm-smoke needs a report path.") }
        enterWorld()
        _ = try await web.callAsyncJavaScript("const {instance}=await WebAssembly.instantiate(await (await fetch('/build/voice.wasm')).arrayBuffer(),{wasi_snapshot_preview1:new Proxy({},{get:()=>()=>0})});globalThis.__warmCodec=instance.exports;return true;", arguments: [:], in: nil, contentWorld: .page)
        _ = try await web.evaluateJavaScript("""
        globalThis.__warmCheck={microphones:0,sockets:0,sends:0,phases:[],wire:[],received:[]};
        const decode=b=>{const a=ArrayBuffer.isView(b)?new Uint8Array(b.buffer,b.byteOffset,b.byteLength):new Uint8Array(b),x=__warmCodec;new Uint8Array(x.memory.buffer,x.vw_frame(),a.length).set(a);const kind=x.vw_decode(a.length),field=x.vw_field(0),epoch=x.vw_field(3),samples=x.vw_field(4);let n=x.vw_string(2);const turn=new TextDecoder().decode(new Uint8Array(x.memory.buffer,x.vw_text(),n));n=x.vw_string(0);const text=new TextDecoder().decode(new Uint8Array(x.memory.buffer,x.vw_text(),n));return {kind,field,epoch,samples,turn,text};};
        const W=WebSocket;globalThis.WebSocket=class extends W {
          constructor(...a){super(...a);this.__local=String(a[0])===coaxialVoiceHost;if(this.__local){__warmCheck.sockets++;globalThis.__warmLast=this;this.addEventListener('message',e=>{if(e.data instanceof ArrayBuffer)__warmCheck.received.push(decode(e.data));});}}
          send(b){if(this.__local){__warmCheck.sends++;const row=decode(b);const last=__warmCheck.wire.at(-1);if(row.kind===3&&last?.kind===3)last.audioFrames=(last.audioFrames||1)+1;else __warmCheck.wire.push(row);}return super.send(b);}
        };
        const original=navigator.mediaDevices.getUserMedia.bind(navigator.mediaDevices);
        globalThis.__warmRestore=()=>{navigator.mediaDevices.getUserMedia=original;globalThis.WebSocket=W;};
        navigator.mediaDevices.getUserMedia=async()=>{__warmCheck.microphones++;throw new Error('Warmup must not request the microphone');};
        true
        """)
        let started = ProcessInfo.processInfo.systemUptime
        try await prepareVoice()
        let warmSeconds = ProcessInfo.processInfo.systemUptime - started
        let idle = try await web.callAsyncJavaScript("""
        const a={...__warmCheck,state:coaxial.voice.prepared()};
        await new Promise(r=>setTimeout(r,250));
        return {...a,idleExtraSends:__warmCheck.sends-a.sends};
        """, arguments: [:], in: nil, contentWorld: .page) as? [String:Any] ?? [:]
        guard idle["microphones"] as? Int == 0, idle["idleExtraSends"] as? Int == 0,
              (idle["state"] as? [String:Any])?["ready"] as? Bool == true else { throw ShellError.failed("Voice prewarm did not become ready without capture: \(idle)") }
        let holdReport = try await web.callAsyncJavaScript("""
        const fixture=new AudioContext();await fixture.resume();
        const result=[];
        const toggle=coaxial.voice.toggle;
        // Observe promise completion, not the hidden window's clamped polling
        // timer. The interval still starts before the actual V key event.
        coaxial.voice.toggle=want=>{const pending=toggle(want);if(want&&want!=='end')pending.then(()=>{globalThis.__warmHoldReady=performance.now();});return pending;};
        try {
        // Pump the realm's two outbox frames (facing, then hold) here as well as
        // in the native harness; an unordered WebKit window throttles timers.
        const key=(type,k)=>{dispatchEvent(new KeyboardEvent(type,{key:k,code:'Key'+k.toUpperCase(),bubbles:true}));for(let i=0;i<2;i++)if(globalThis.__forgeFrame){const f=__forgeFrame;globalThis.__forgeFrame=null;f(performance.now());}};
        for(let i=0;i<3;i++){
          await coaxial.voice.warm();
          const before=__warmCheck.sockets;
          const capture=fixture.createMediaStreamDestination();
          navigator.mediaDevices.getUserMedia=async()=>{__warmCheck.microphones++;return capture.stream;};
          const phaseAt=globalThis.coaxialVoice?.length||0;
          globalThis.__warmHoldReady=0;
          const started=performance.now();
          if(i===0){key('keydown','v');const end=performance.now()+1500;while(!(globalThis.coaxialVoice||[]).slice(phaseAt).some(s=>s.startsWith('listening|'))&&performance.now()<end)await new Promise(r=>setTimeout(r,1));}
          else await coaxial.voice.toggle('on');
          const state=coaxial.voice.prepared();
          result.push({milliseconds:__warmHoldReady?__warmHoldReady-started:10000,actualV:i===0,extraSockets:__warmCheck.sockets-before,connecting:(globalThis.coaxialVoice||[]).slice(phaseAt).some(s=>s.startsWith('connecting|')),...state});
          await new Promise(r=>setTimeout(r,80));
          if(i===0)key('keyup','v');else coaxial.voice.toggle(false);
          const released=performance.now()+500;
          while(capture.stream.getTracks().some(t=>t.readyState==='live')&&performance.now()<released)await new Promise(r=>setTimeout(r,5));
          if(capture.stream.getTracks().some(t=>t.readyState==='live'))throw new Error('Capture survived cancellation');
          await new Promise(r=>setTimeout(r,100));
        }
        return {rows:result};
        } catch(e) {return {rows:result,error:String(e),stack:e.stack,state:coaxial.voice.prepared()};}
        finally {coaxial.voice.toggle(false);coaxial.voice.toggle=toggle;await fixture.close();}
        """, arguments: [:], in: nil, contentWorld: .page) as? [String:Any] ?? [:]
        let holds = holdReport["rows"] as? [[String:Any]] ?? []
        let wav = try Data(contentsOf: Bundle.main.resourceURL!.appendingPathComponent("hello-foreman.wav")).base64EncodedString()
        let crates = try Data(contentsOf: Bundle.main.resourceURL!.appendingPathComponent("add-two-giant-crates.wav")).base64EncodedString()
        let speech = try await web.callAsyncJavaScript("""
        const originalFinal=globalThis.coaxialVoiceFinal;
        const originalMedia=navigator.mediaDevices.getUserMedia.bind(navigator.mediaDevices);
        const fixture=new AudioContext();await fixture.resume();
        try {
          let transcript='',tracksClosed=true,heldDuringPause=false;const transcripts=[];
          globalThis.coaxialVoiceFinal=e=>{transcript=e.text;return true;};
          const buffer=await fixture.decodeAudioData(Uint8Array.from(atob('\(wav)'),c=>c.charCodeAt(0)).buffer);
          const crates=await fixture.decodeAudioData(Uint8Array.from(atob('\(crates)'),c=>c.charCodeAt(0)).buffer);
          const gap=2.4,leading=1,rate=fixture.sampleRate;
          const whole=fixture.createBuffer(1,Math.ceil((leading+crates.duration+gap+buffer.duration+0.3)*rate),rate);
          whole.getChannelData(0).set(crates.getChannelData(0),Math.round(leading*rate));
          whole.getChannelData(0).set(buffer.getChannelData(0),Math.round((leading+crates.duration+gap)*rate));
          const key=type=>{dispatchEvent(new KeyboardEvent(type,{key:'v',code:'KeyV',bubbles:true}));for(let i=0;i<2;i++)if(globalThis.__forgeFrame){const f=__forgeFrame;globalThis.__forgeFrame=null;f(performance.now());}};
          for(let i=0;i<2;i++){
            transcript='';const source=fixture.createBufferSource();source.buffer=i===0?whole:buffer;
            const capture=fixture.createMediaStreamDestination();source.connect(capture);
            navigator.mediaDevices.getUserMedia=async()=>capture.stream;
            await coaxial.voice.warm();coaxial.voice.face('Coding agent');
            if(i===0){const at=globalThis.coaxialVoice.length;key('keydown');const end=performance.now()+1500;while(!globalThis.coaxialVoice.slice(at).some(s=>s.startsWith('listening|'))&&performance.now()<end)await new Promise(r=>setTimeout(r,5));}
            else await coaxial.voice.toggle('hold');
            const played=new Promise(r=>{source.onended=r;source.start();});
            if(i===0){
              await new Promise(r=>setTimeout(r,(leading+crates.duration+gap-0.3)*1000));
              const state=coaxial.voice.prepared();heldDuringPause=state.running&&state.microphone&&!transcript;
              if(!heldDuringPause)throw Error('Held V finalized or closed capture during the pause');
            }
            await played;
            await new Promise(r=>setTimeout(r,300));
            if(i===0)key('keyup');else await coaxial.voice.toggle('end');
            const end=performance.now()+20000;while(!transcript && performance.now()<end)await new Promise(r=>setTimeout(r,50));
            transcripts.push(transcript);tracksClosed=tracksClosed&&capture.stream.getTracks().every(t=>t.readyState==='ended');
            coaxial.voice.toggle(false);await coaxial.voice.warm();
          }
          const late=fixture.createMediaStreamDestination();let deliver;
          navigator.mediaDevices.getUserMedia=()=>new Promise(r=>{deliver=r});
          const starting=coaxial.voice.toggle('on');coaxial.voice.toggle(false);deliver(late.stream);await starting;
          const cancelled=late.stream.getTracks().every(t=>t.readyState==='ended');
          await new Promise(r=>setTimeout(r,100));await coaxial.voice.warm();
          const recovery=fixture.createMediaStreamDestination();navigator.mediaDevices.getUserMedia=async()=>recovery.stream;
          await coaxial.voice.toggle('on');const sockets=__warmCheck.sockets;__warmLast.close();
          await new Promise(r=>setTimeout(r,30));coaxial.voice.toggle(false);await coaxial.voice.warm();
          await new Promise(r=>setTimeout(r,650));
          const reconnected=coaxial.voice.prepared().ready && __warmCheck.sockets===sockets+1 && recovery.stream.getTracks().every(t=>t.readyState==='ended');
          const {startVoice}=await import('/coaxial/voice.js');const phases=[];
          const cold=startVoice({send:(_,s)=>phases.push(s),route:'everglade',wasmUrl:'/build/voice.wasm'});
          const fresh=fixture.createMediaStreamDestination();navigator.mediaDevices.getUserMedia=async()=>fresh.stream;
          await cold.toggle('on');const until=performance.now()+3000;
          while(!phases.some(s=>s.startsWith('listening|')) && performance.now()<until)await new Promise(r=>setTimeout(r,20));
          const coldReady=phases.some(s=>s.startsWith('listening|'));cold.toggle(false);
          return {transcript,transcripts,heldDuringPause,heldSeconds:whole.duration,cancelled,coldReady,reconnected,tracksClosed,wire:__warmCheck.wire,received:__warmCheck.received};
        } catch(e) {return {error:String(e)};}
        finally {coaxial.voice.toggle(false);navigator.mediaDevices.getUserMedia=originalMedia;globalThis.coaxialVoiceFinal=originalFinal;await fixture.close();__warmRestore();}
        """, arguments: [:], in: nil, contentWorld: .page) as? [String:Any] ?? [:]
        let transcript = (speech["transcript"] as? String ?? "").lowercased()
        let transcripts = speech["transcripts"] as? [String] ?? []
        guard transcripts.count == 2, transcripts.first?.range(of: "add (two|2|to) giant crates",options:[.regularExpression,.caseInsensitive]) != nil, speech["heldDuringPause"] as? Bool == true, transcripts.allSatisfy({$0.lowercased().contains("hello") && $0.lowercased().contains("building today")}), transcript.contains("hello"), transcript.contains("building today"), speech["cancelled"] as? Bool == true,
              speech["coldReady"] as? Bool == true, speech["reconnected"] as? Bool == true, speech["tracksClosed"] as? Bool == true else { throw ShellError.failed("Prepared voice speech/cancellation regression: \(speech)") }
        try JSONSerialization.data(withJSONObject:["warmSeconds":warmSeconds,"idle":idle,"holds":holds,"speech":speech,"hardwareMicrophone":false],options:[.prettyPrinted,.sortedKeys]).write(to:report,options:.atomic)
        guard holdReport["error"] == nil, holds.count == 3, holds.allSatisfy({ $0["extraSockets"] as? Int == 0 && $0["connecting"] as? Bool == false && $0["connected"] as? Bool == true && $0["microphone"] as? Bool == true && ($0["milliseconds"] as? Double ?? 10000) < 250 }) else {
            throw ShellError.failed("A prepared voice hold still waited for a connection: \(holdReport)")
        }
        stopChildren();NSApp.terminate(nil)
    }

    func warmVoiceIfNeeded() {
        guard setupPane != nil, window != nil else { return }
        let interactive = !smoke && launchReport == nil && microphoneReport == nil && frameProfileURL == nil && setupProfileURL == nil && offlineReportURL == nil
        guard forge_voice_warm(!stopping && pageURL != nil, window.isVisible && !window.isMiniaturized,
                               microphoneStatus() == .authorized, fgs_status(1) > 0, interactive, voiceWarmup != nil, voiceWarmReady) else { return }
        Task { try? await prepareVoice() }
    }
    func prepareVoice() async throws {
        if let pending = voiceWarmup { try await pending.value; return }
        let epoch = voiceWarmEpoch
        let task = Task {
            fgs_voice_ready(1); setupPane.update()
            try await ensureVoiceHost()
            guard !stopping, epoch == voiceWarmEpoch else { throw CancellationError() }
            let ready = try await web.callAsyncJavaScript("""
            const end=performance.now()+8000;
            while(!globalThis.coaxial?.voice && performance.now()<end)await new Promise(r=>setTimeout(r,20));
            if(!globalThis.coaxial?.voice)throw new Error('Voice page is not ready');
            return await coaxial.voice.warm();
            """, arguments: [:], in: nil, contentWorld: .page) as? Bool == true
            guard ready, !stopping, epoch == voiceWarmEpoch else { throw CancellationError() }
            voiceWarmReady = true; fgs_voice_ready(2); setupPane.update()
        }
        voiceWarmup = task
        defer { if epoch == voiceWarmEpoch { voiceWarmup = nil } }
        do { try await task.value }
        catch { if epoch == voiceWarmEpoch && !stopping { voiceWarmReady = false; fgs_voice_ready(3); setupPane.update(); traceVoice("warm:error:\(error.localizedDescription)") }; throw error }
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
    func cancelVoice(_ reason: String = "cancel") {
        traceVoice("capture:cancel reason=\(reason)")
        voiceCaptureEpoch += 1; voiceCaptureRequest = nil
        fgv_cancel(); _ = web?.evaluateJavaScript("globalThis.coaxialVoiceCancel?.(); true")
    }
    func armVoiceCapture() async throws {
        let ticket = fgv_arm()
        guard ticket > 0 else { throw ShellError.failed("Voice capture generation is exhausted; reopen the app.") }
        _ = try await web.evaluateJavaScript("globalThis.coaxialCoderVoiceTicket=\(ticket); true")
    }
    // Permission sheets and other AppKit panels take key focus. Only leaving
    // the app (or explicit hide/minimize/setup) cancels its live capture.
    func applicationDidBecomeActive(_ notification: Notification) { refreshMicrophoneSetup() }
    func windowDidBecomeKey(_ notification: Notification) { metalWorld?.focus(true); refreshMicrophoneSetup() }
    func applicationDidResignActive(_ notification: Notification) { if !microphonePrompt { cancelVoice("focus-lost") } }
    func applicationDidHide(_ notification: Notification) { cancelVoice("app-hidden"); updateWorldVisibility() }
    func applicationDidUnhide(_ notification: Notification) { updateWorldVisibility() }
    func windowDidChangeOcclusionState(_ notification: Notification) { updateWorldVisibility() }
    func windowDidResignKey(_ notification: Notification) { metalWorld?.focus(false); coderHeldKeys.removeAll() }
    func windowDidMiniaturize(_ notification: Notification) { cancelVoice("window-minimized"); updateWorldVisibility() }
    func windowDidDeminiaturize(_ notification: Notification) { updateWorldVisibility() }
    func microphoneStatus() -> AVAuthorizationStatus {
        microphoneAuthorization?() ?? AVCaptureDevice.authorizationStatus(for: .audio)
    }
    func updateMicrophoneSetup() {
        guard setupPane != nil else { return }
        fgs_microphone(Int64(microphoneStatus().rawValue), microphonePrompt)
        setupPane.update()
        warmVoiceIfNeeded()
    }
    func refreshMicrophoneSetup() {
        guard setupPane != nil, window != nil else { return }
        updateMicrophoneSetup()
        let interactive = !smoke && launchReport == nil && microphoneReport == nil && frameProfileURL == nil && setupProfileURL == nil && offlineReportURL == nil
        guard fm_setup_request(!stopping && window.isVisible && !window.isMiniaturized, NSApp.isActive,
                               !setupPane.isHidden, fgs_completed(), Int64(microphoneStatus().rawValue), microphonePrompt, interactive) else { return }
        Task { _ = await authorizeMicrophone() }
    }
    func authorizeMicrophone() async -> Bool {
        if let pending = microphonePermissionTask { let granted = await pending.value; return granted && microphoneStatus() == .authorized }
        guard microphoneStatus() == .notDetermined else {
            updateMicrophoneSetup(); return microphoneStatus() == .authorized
        }
        microphonePrompt = true; updateMicrophoneSetup()
        let task = Task { @MainActor in
            if let request = microphoneAccessRequest { return await request() }
            return await AVCaptureDevice.requestAccess(for: .audio)
        }
        microphonePermissionTask = task
        let granted = await task.value
        microphonePermissionTask = nil; microphonePrompt = false; updateMicrophoneSetup()
        return granted && microphoneStatus() == .authorized
    }
    func microphoneSetupPressed() {
        if fgs_status(3) == 1 { refreshMicrophoneSetup() }
        else if fgs_status(3) == 2, let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone") { NSWorkspace.shared.open(url) }
    }
    var voiceAvailable: Bool {
        fm_capture(!stopping && sceneReady && setupPane.isHidden,
                   window.isVisible && !window.isMiniaturized, NSApp.isActive)
    }
    func trustedVoiceFrame(_ frame: WKFrameInfo) -> Bool {
        let origin = frame.securityOrigin
        return frame.isMainFrame && origin.protocol == "http" && origin.host == "127.0.0.1" && origin.port == pageURL?.port
    }
    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage, replyHandler: @escaping (Any?, String?) -> Void) {
        guard message.name == "forgeVoiceCapture", trustedVoiceFrame(message.frameInfo),
              let body = message.body as? [String: Any], body["schema"] as? String == "aethyrforge.microphone-capture.v1",
              let capture = body["capture"] as? Int, capture > 0, capture <= 9007199254740991 else {
            replyHandler(nil, "Invalid local microphone request."); return
        }
        if body["kind"] as? String == "cancel" {
            if voiceCaptureRequest == capture { voiceCaptureEpoch += 1; voiceCaptureRequest = nil; fgv_cancel() }
            replyHandler(true, nil); return
        }
        let fixture = voiceSmoke || voiceWarmSmoke
        guard body["kind"] as? String == "prepare", (!smoke || fixture), (fixture || voiceAvailable),
              (fixture || AVCaptureDevice.authorizationStatus(for: .audio) == .authorized) else {
            replyHandler(nil, "Voice stopped. Return to Aethyrforge and hold TALK again."); return
        }
        voiceCaptureEpoch += 1; voiceCaptureRequest = capture
        let epoch = voiceCaptureEpoch
        Task {
            do {
                if microphoneReport != nil && microphoneCheckFailStartup {
                    microphoneCheckFailStartup = false
                    throw ShellError.failed("Local voice test startup failure.")
                }
                if microphoneReport != nil && microphoneCheckDelay { try await Task.sleep(for: .milliseconds(500)) }
                try await ensureVoiceHost()
                guard epoch == voiceCaptureEpoch, (fixture || voiceAvailable) else { throw ShellError.failed("Voice stopped. Hold TALK again.") }
                try await armVoiceCapture()
                guard epoch == voiceCaptureEpoch, (fixture || voiceAvailable) else { throw ShellError.failed("Voice stopped. Hold TALK again.") }
                replyHandler(true, nil)
            } catch { replyHandler(nil, error.localizedDescription) }
        }
    }

    // SDK acceptance: real WebKit + physical microphone permission. Stop each
    // track immediately; no audio graph, recording or recognition receives it.
    func microphoneCheck() async throws {
        guard let microphoneReport else { return }
        do {
            showSetup()
            let prepared = await authorizeMicrophone()
            guard prepared, setupPane.microphoneStatus.stringValue.contains("Microphone allowed"), voiceHost == nil, microphoneDecisions == 0 else {
                throw ShellError.failed("Microphone permission was not ready during setup before the first V capture.")
            }
            enterWorld()
            guard setupPane.isHidden else { throw ShellError.failed("Microphone check needs the ready world.") }
            window.makeKeyAndOrderFront(nil); NSApp.activate(ignoringOtherApps: true)
            try await Task.sleep(for: .milliseconds(300))
            // Reproduce the reported state: visible foreground app, no key window.
            window.resignKey()
            let withoutKey = !window.isKeyWindow
            let first = try await web.callAsyncJavaScript("""
            const stream=await navigator.mediaDevices.getUserMedia({audio:true});
            const live=stream.getAudioTracks().some(t=>t.readyState==='live');
            stream.getTracks().forEach(t=>t.stop());
            await coaxialVoicePrepareCapture(101);
            const ticket=coaxialCoderVoiceTicket;
            await coaxialVoiceCancelCapture(101);
            return {live,ticket,closed:stream.getTracks().every(t=>t.readyState==='ended')};
            """, arguments: [:], in: nil, contentWorld: .page) as? [String: Any] ?? [:]
            microphoneCheckFailStartup = true
            let second = try await web.callAsyncJavaScript("""
            const stream=await navigator.mediaDevices.getUserMedia({audio:true});
            stream.getTracks().forEach(t=>t.stop());
            let failed=false;try{await coaxialVoicePrepareCapture(102);}catch(e){failed=String(e).includes('test startup failure');}
            const again=await navigator.mediaDevices.getUserMedia({audio:true});
            const live=again.getAudioTracks().some(t=>t.readyState==='live');
            again.getTracks().forEach(t=>t.stop());
            await coaxialVoicePrepareCapture(103);
            const ticket=coaxialCoderVoiceTicket;
            await coaxialVoiceCancelCapture(103);
            return {live,ticket,failed,closed:again.getTracks().every(t=>t.readyState==='ended')};
            """, arguments: [:], in: nil, contentWorld: .page) as? [String: Any] ?? [:]
            microphoneCheckDelay = true
            let cancelled = try await web.callAsyncJavaScript("""
            const pending=coaxialVoicePrepareCapture(104);
            await coaxialVoiceCancelCapture(104);
            try{await pending;return false;}catch(e){return String(e).includes('Voice stopped');}
            """, arguments: [:], in: nil, contentWorld: .page) as? Bool == true
            let one = first["ticket"] as? Int ?? 0, two = second["ticket"] as? Int ?? 0
            guard withoutKey, first["live"] as? Bool == true, second["live"] as? Bool == true,
                  first["closed"] as? Bool == true, second["closed"] as? Bool == true,
                  second["failed"] as? Bool == true, one > 0, two > one, cancelled, microphoneDecisions > 0 else {
                throw ShellError.failed("Microphone regression failed: \(first), \(second), cancelled=\(cancelled), key=\(!withoutKey).")
            }
            let result: [String: Any] = ["authorized": AVCaptureDevice.authorizationStatus(for: .audio) == .authorized,
                "realMicrophone": true, "noKeyWindow": withoutKey, "tracksClosed": true,
                "repeatCapture": true, "freshTickets": [one,two], "startupFailureRecovered": true,
                "cancelledPreparation": cancelled, "permissionDecisions": microphoneDecisions,
                "audioRecordedOrSent": false, "authorizedBeforeCapture": true]
            try JSONSerialization.data(withJSONObject: result, options: [.prettyPrinted, .sortedKeys]).write(to: microphoneReport)
        } catch {
            try? Data(error.localizedDescription.utf8).write(to: microphoneReport.appendingPathExtension("error"))
        }
        NSApp.terminate(nil)
    }
    func traceVoice(_ line: String) {
        guard let dataURL, line.utf8.count <= 1024 else { return }
        let path = dataURL.appendingPathComponent("voice-access.log")
        if !fm.fileExists(atPath: path.path) { fm.createFile(atPath: path.path, contents: nil) }
        if let file = try? FileHandle(forWritingTo: path) { defer { try? file.close() }; _ = try? file.seekToEnd(); try? file.write(contentsOf: Data((Date().ISO8601Format() + " " + line + "\n").utf8)) }
    }
    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
        let origin = message.frameInfo.securityOrigin
        guard message.frameInfo.isMainFrame, origin.protocol == "http", origin.host == "127.0.0.1", origin.port == pageURL?.port,
              let line = message.body as? String else { return }
        if message.name == "forgeMetalMessage", let bytes = line.data(using: .utf8),
           let packet = try? JSONSerialization.jsonObject(with: bytes) as? [String: Any],
           let kind = packet["kind"] as? Int32, let text = packet["text"] as? String { metalWorld?.message(kind, text); return }
        if message.name == "forgeVoiceStatus" {
            traceVoice(line)
            if line == "warm:ready" { voiceWarmReady = true; fgs_voice_ready(2); setupPane.update() }
            if line == "warm:lost" { voiceWarmReady = false; fgs_voice_ready(3); setupPane.update(); warmVoiceIfNeeded() }
            return
        }
        guard message.name == "forgeVoiceCoder", line.utf8.count <= 131072 else { return }
        let available = voiceSmoke || voiceAvailable
        let bytes = Array(line.utf8)
        let action = bytes.withUnsafeBufferPointer { fgv_message($0.baseAddress, Int64($0.count), available, chatPane.busy) }
        if action == 1 {
            var text = [UInt8](repeating: 0, count: 16384)
            let n = fgv_copy(&text, Int64(text.count))
            if n > 0 { chatPane.startTurn(String(decoding: text.prefix(Int(n)), as: UTF8.self), voice: true) }
        } else if action == 2 {
            let notice = "Coder is busy. This voice message was not sent.\n"
            chatPane.append(notice, color: .systemOrange); chatPane.appendMessage(notice)
            cancelVoice()
        }
    }

    func voiceSmokeCheck(_ resources: URL) async throws {
        guard let report, voiceHost == nil else { throw ShellError.failed("Voice loaded before it was requested.") }
        enterWorld()
        guard setupPane.isHidden else { throw ShellError.failed("Voice-to-Coder needs the ready world.") }
        let root = report.deletingLastPathComponent()
        chatPane.home = root; chatPane.cliOverride = resources.appendingPathComponent("openagents")
        chatPane.cwd = root.appendingPathComponent("workspace")
        try fm.createDirectory(at: chatPane.cwd, withIntermediateDirectories: true)
        let config = root.appendingPathComponent(".openagents/coder/router.json")
        try fm.createDirectory(at: config.deletingLastPathComponent(), withIntermediateDirectories: true)
        if !fm.fileExists(atPath: config.path) { try fm.copyItem(at: fm.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/coder/router.json"), to: config) }
        try fm.createDirectory(at: root.appendingPathComponent(".openagents/aethyrforge"), withIntermediateDirectories: true)
        chatPane.input.string = "UNSENT_TYPED_DRAFT"
        try await ensureVoiceHost()
        let fixture = try Data(contentsOf: resources.appendingPathComponent("hello-foreman.wav")).base64EncodedString()
        _ = try await web.evaluateJavaScript("""
        globalThis.__voiceCheck = []; globalThis.__voiceError = ''; globalThis.__voiceSockets = 0; globalThis.__voiceFinals = [];
        globalThis.__voiceRuntimeErrors = [];
        addEventListener('error',e=>__voiceRuntimeErrors.push(String(e.message)));
        addEventListener('unhandledrejection',e=>__voiceRuntimeErrors.push(String(e.reason)));
        globalThis.__voiceDeliverFinal = globalThis.coaxialVoiceFinal;
        globalThis.coaxialVoiceFinal = e => {__voiceFinals.push(e); return __voiceDeliverFinal(e);};
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
          globalThis.__voiceFixtureEnded = false; source.onended = () => {__voiceFixtureEnded = true;};
          globalThis.__voiceClient = startVoice({send:(k,t)=>__voiceCheck.push(t),route:'everglade',wasmUrl:'/build/voice.wasm'});
          __voiceClient.face('Coding agent');
          globalThis.__voiceRestore = () => {md.getUserMedia=original;};
        })().catch(e=>{globalThis.__voiceError=String(e)}); true;
        """)
        let readyDeadline = Date().addingTimeInterval(15)
        while Date() < readyDeadline {
            if try await web.evaluateJavaScript("!!globalThis.__voiceClient") as? Bool == true { break }
            try await Task.sleep(for: .milliseconds(50))
        }
        _ = try await web.evaluateJavaScript("__voiceClient.toggle('on'); true")
        let listenDeadline = Date().addingTimeInterval(15)
        var listening = false
        while Date() < listenDeadline {
            if try await web.evaluateJavaScript("__voiceCheck.some(t=>t.startsWith('listening|'))") as? Bool == true { listening = true; break }
            try await Task.sleep(for: .milliseconds(100))
        }
        guard listening else { throw ShellError.failed("Browser voice did not listen: \(String(describing: try await web.evaluateJavaScript("__voiceError || JSON.stringify(__voiceCheck)")))") }
        _ = try await web.evaluateJavaScript("__voiceFixture.start(); true")
        let audioDeadline = Date().addingTimeInterval(15)
        while Date() < audioDeadline {
            if try await web.evaluateJavaScript("__voiceFixtureEnded") as? Bool == true { break }
            try await Task.sleep(for: .milliseconds(100))
        }
        try await Task.sleep(for: .milliseconds(800))
        _ = try await web.evaluateJavaScript("__voiceClient.toggle('end'); true")
        let finalDeadline = Date().addingTimeInterval(30)
        var transcript = ""
        while Date() < finalDeadline {
            transcript = try await web.evaluateJavaScript("__voiceFinals[0]?.text || ''") as? String ?? ""
            if !transcript.isEmpty { break }
            try await Task.sleep(for: .milliseconds(100))
        }
        let cancelled = try await web.evaluateJavaScript("__voiceCancelled === true") as? Bool == true
        _ = try await web.evaluateJavaScript("__voiceClient.toggle(false); __voiceRestore?.(); __voiceFixtureContext.close(); true")
        guard cancelled, transcript.lowercased().contains("hello foreman"), transcript.lowercased().contains("building today") else {
            let detail = try await web.evaluateJavaScript("JSON.stringify({phases:__voiceCheck,error:__voiceError,runtime:__voiceRuntimeErrors,finals:__voiceFinals})") as? String ?? ""
            throw ShellError.failed("The local browser voice fixture failed: \(transcript). Cancellation: \(cancelled). \(detail)")
        }
        let coderDeadline = Date().addingTimeInterval(90)
        var deniedRequests = 0
        while (chatPane.busy || chatPane.session.isEmpty) && Date() < coderDeadline {
            if chatPane.pending { chatPane.reject(); deniedRequests += 1 }
            if deniedRequests > 3 { chatPane.stopTurn(); break }
            try await Task.sleep(for: .milliseconds(50))
        }
        guard !chatPane.busy, chatPane.lastEnding.hasPrefix("finished"), chatPane.liveChunks > 0, chatPane.streamedWithoutComposer,
              !chatPane.composerOpen, chatPane.input.string == "UNSENT_TYPED_DRAFT", !chatPane.session.isEmpty else {
            throw ShellError.failed("Final Parakeet words did not finish a routed, streamed Coder turn with the bar closed: \(chatPane.lastEnding)")
        }
        let session = chatPane.session
        let log = try String(contentsOf: chatPane.sessions.appendingPathComponent(session + ".jsonl"), encoding: .utf8)
        guard log.contains("\"schema\":\"coder.route.v2\""), log.contains("\"ok\":true"), log.contains("\"kind\":\"reply\""), !log.contains("reply_delta") else { throw ShellError.failed("Routed voice turn did not retain a canonical whole-reply log.") }
        _ = try await web.evaluateJavaScript("__voiceDeliverFinal(__voiceFinals[0]); true")
        try await Task.sleep(for: .milliseconds(200))
        guard !chatPane.busy, chatPane.session == session else { throw ShellError.failed("A duplicate final started another Coder turn.") }
        let result: [String: Any] = ["local": true, "transcript": transcript, "lateMicrophoneCancelled": cancelled,
            "voicePID": voiceHost!.processIdentifier, "roomPID": room!.processIdentifier, "pagePID": page!.processIdentifier,
            "startupModelLoaded": false, "coderSession": session, "engine": chatPane.selectedEngine,
            "liveChunks": chatPane.liveChunks, "streamedWithBarClosed": chatPane.streamedWithoutComposer,
            "draftPreserved": true, "duplicateIgnored": true, "canonicalLog": true, "fixtureToolRequestsDenied": deniedRequests]
        try JSONSerialization.data(withJSONObject: result, options: [.prettyPrinted, .sortedKeys]).write(to: report, options: .atomic)
        stopChildren(); NSApp.terminate(nil)
    }

    func setupSmokeCheck() async throws {
        guard let report, fgs_can_enter(sceneReady), fgs_status(1) == 1 else { throw ShellError.failed("Setup did not discover the CLI and installed shared voice model without hashing.") }
        // Fake only the macOS authorization transport, never the Coax policy or
        // a microphone stream. Exercise the first-run sheet and a racing V check.
        var status = AVAuthorizationStatus.notDetermined
        var requests = 0
        microphoneAuthorization = { status }
        microphoneAccessRequest = {
            requests += 1
            try? await Task.sleep(for: .milliseconds(80))
            status = .authorized; return true
        }
        refreshMicrophoneSetup()
        guard setupPane.microphone.isEnabled, fgs_status(3) == 1 else { throw ShellError.failed("Setup has no microphone permission action.") }
        let first = Task { await authorizeMicrophone() }
        try await Task.sleep(for: .milliseconds(20))
        guard microphonePrompt, !setupPane.microphone.isEnabled else { throw ShellError.failed("Setup must show an in-flight permission request.") }
        let second = await authorizeMicrophone()
        let granted = await first.value
        guard granted, second, requests == 1, setupPane.microphoneStatus.stringValue.contains("Microphone allowed"), voiceHost == nil, microphoneDecisions == 0 else {
            throw ShellError.failed("Setup did not acquire permission before voice, or duplicated the system request.")
        }
        status = .denied; refreshMicrophoneSetup()
        let denied = await authorizeMicrophone()
        guard !denied, requests == 1, setupPane.microphone.isEnabled, setupPane.microphone.title == "Open Settings" else { throw ShellError.failed("Denied permission must link to Settings without asking again.") }
        microphoneAuthorization = nil; microphoneAccessRequest = nil; refreshMicrophoneSetup()
        let skipped = setupPane.isHidden
        guard skipped == fgs_completed(), setupPane.enter.isEnabled else { throw ShellError.failed("Launch did not respect the saved setup completion.") }
        enterWorld()
        guard setupPane.isHidden, fgs_completed(), fm.fileExists(atPath: settingsURL.path) else { throw ShellError.failed("Setup could not enter the world and keep its choices.") }
        showSetup()
        refreshSetup(verify: false); await inventoryTask?.value
        guard !setupPane.isHidden else { throw ShellError.failed("Manually opened setup disappeared after refresh.") }
        let result: [String: Any] = ["ready": sceneReady, "cli": fgs_status(0) == 1, "voiceInstalled": fgs_status(1) > 0, "voiceVerified": fgs_status(1) == 2,
            "localAgents": fgs_count(-1), "selectedAgent": fgs_selected(), "voicePath": setupText(6),
            "setupVisible": !setupPane.isHidden, "setupSkippedOnLaunch": skipped, "settingsSaved": true, "firstFrameSeconds": firstFrameSeconds,
            "roomPID": room!.processIdentifier, "pagePID": page!.processIdentifier,
            "microphoneBeforeVoice": true, "permissionRequests": requests, "concurrentPermissionShared": true,
            "deniedPermissionSettings": true, "microphoneOpened": false, "microphoneStatus": setupPane.microphoneStatus.stringValue]
        try await captureCanvas(report)
        try JSONSerialization.data(withJSONObject: result, options: [.prettyPrinted, .sortedKeys]).write(to: report, options: .atomic)
        stopChildren(); NSApp.terminate(nil)
    }

    func makeMetalWorld(frame: NSRect) -> ForgeMetalWorld {
        let view = ForgeMetalWorld(frame: frame)
        view.autoresizingMask = [.width, .height]
        view.failed = { [weak self] in self?.fail($0) }
        view.action = { [weak self] code, text in self?.metalAction(code, text) }
        return view
    }
    func previewTitle() -> String {
        var bytes: [UInt8] = []
        for i in 0..<64 {
            let byte = fwp_label_byte(metalWorld == nil ? 0:1, Int64(i))
            if byte < 0 { break }; bytes.append(UInt8(byte))
        }
        return String(decoding: bytes, as: UTF8.self)
    }
    func validateMenuItem(_ item: NSMenuItem) -> Bool {
        if item.action == #selector(toggleWebPreview) {
            item.title = previewTitle()
            return fwp_target(worldBackend, metalWorld == nil ? 0:1, sceneReady && !stopping, previewSwitchTask != nil) >= 0
        }
        if item.action == #selector(reloadWorld) { return !stopping && previewSwitchTask == nil }
        return true
    }
    @objc func toggleWebPreview() {
        let destination = fwp_target(worldBackend, metalWorld == nil ? 0:1, sceneReady && !stopping, previewSwitchTask != nil)
        guard destination >= 0 else { return }
        previewSwitchTask = Task {
            defer { previewSwitchTask = nil }
            do { try await switchWorldView(destination) }
            catch { if !Task.isCancelled && !stopping { fail(error.localizedDescription) } }
        }
    }
    func switchWorldView(_ destination: Int64) async throws {
        guard let resources = Bundle.main.resourceURL, let pageURL else { throw ShellError.failed("World preview is not ready.") }
        sceneReady = false; setupPane.sceneReady = false; worldPauseWanted = nil
        cancelVoice()
        voiceWarmEpoch += 1; voiceWarmup?.cancel(); voiceWarmup = nil; voiceWarmReady = false; fgs_voice_ready(0)
        notice.stringValue = "Loading \(destination == 0 ? "web preview":"native world")…"; notice.isHidden = false
        window.makeFirstResponder(nil)
        if destination == 0 {
            if let previous = metalWorld {
                previous.focus(false)
                let audio = previous.audio?.source
                previous.close(); previous.removeFromSuperview(); metalWorld = nil
                lastPreviewRetired = ["ledgerLive":mf_closed_info(0),"ledgerWords":mf_closed_info(1),"mediaBytes":mf_closed_info(2),"audio":audio?.observations() ?? [:]]
            }
            web.isHidden = false
            web.load(URLRequest(url: pageURL))
        } else {
            _ = try? await web.evaluateJavaScript("globalThis.coaxial?.setWorldPaused(true); true")
            let auxiliary = URL(string:"http://127.0.0.1:\(pageURL.port!)/metal-voice.html")!
            web.load(URLRequest(url:auxiliary))
            let end = ProcessInfo.processInfo.systemUptime + 10
            while web.url != auxiliary || web.isLoading {
                try Task.checkCancellation()
                guard ProcessInfo.processInfo.systemUptime < end else { throw ShellError.failed("The web preview did not close its document.") }
                try await Task.sleep(for:.milliseconds(20))
            }
            web.isHidden = true
            let view = makeMetalWorld(frame:web.frame)
            window.contentView?.addSubview(view, positioned:.above, relativeTo:web); metalWorld = view
            try view.open(resources:resources,storage:dataURL,page:pageURL,local:metalLocalOrigin,muted:smoke || CommandLine.arguments.contains("--mute-audio"))
        }
        try await waitForScene()
        try Task.checkCancellation()
        sceneReady = true; setupPane.sceneReady = true; notice.isHidden = true
        updateWorldVisibility(); warmVoiceIfNeeded()
        if setupPane.isHidden { window.makeFirstResponder(worldView) }
    }

    @objc func reloadWorld() {
        guard !stopping, previewSwitchTask == nil, pageURL != nil else { return }
        sceneReady = false; worldPauseWanted = nil
        cancelVoice()
        voiceWarmEpoch += 1; voiceWarmup?.cancel(); voiceWarmup = nil; voiceWarmReady = false; fgs_voice_ready(0)
        if let metalWorld, let resources = Bundle.main.resourceURL {
            do {
                try metalWorld.open(resources: resources, storage: dataURL, page: pageURL, local: metalLocalOrigin, muted: smoke || CommandLine.arguments.contains("--mute-audio"))
                Task { do { try await waitForScene(); sceneReady = true; setupPane.sceneReady = true; updateWorldVisibility() } catch { fail(error.localizedDescription) } }
            } catch { fail(error.localizedDescription) }
        } else { web.reload() }
    }
    @objc func openWorldFolder() { if let dataURL { NSWorkspace.shared.open(dataURL) } }
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
    func applicationWillTerminate(_ notification: Notification) { startup?.cancel(); stopChildren() }
    func stopChildren() {
        stopping = true
        previewSwitchTask?.cancel()
        metalWorld?.close()
        voiceWarmEpoch += 1; voiceWarmup?.cancel(); voiceWarmup = nil; voiceWarmReady = false; fgs_voice_ready(0)
        cancelVoice()
        chatPane?.shutdown()
        framePump?.cancel()
        inventoryTask?.cancel()
        sceneContextTask?.cancel()
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
    func webView(_ webView: WKWebView, didStartProvisionalNavigation navigation: WKNavigation!) { sceneReady = false; worldPauseWanted = nil }
    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        guard previewSwitchTask == nil else { return }
        warmVoiceIfNeeded()
        if firstFrameSeconds > 0 {
            Task { do { try await waitForScene(); sceneReady = true; setupPane.sceneReady = true; updateWorldVisibility() } catch { if !stopping { fail(error.localizedDescription) } } }
        }
    }
    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) { fail("The world view stopped. Quit and reopen Aethyrforge.") }
    func webView(_ webView: WKWebView, requestMediaCapturePermissionFor origin: WKSecurityOrigin, initiatedByFrame frame: WKFrameInfo, type: WKMediaCaptureType, decisionHandler: @escaping (WKPermissionDecision) -> Void) {
        traceVoice("microphone request: main=\(frame.isMainFrame) local=\(origin.host == "127.0.0.1" && origin.port == pageURL?.port) key=\(window.isKeyWindow) setup=\(!setupPane.isHidden) authorization=\(AVCaptureDevice.authorizationStatus(for: .audio).rawValue)")
        guard type == .microphone, trustedVoiceFrame(frame), !smoke, !stopping else { decisionHandler(.deny); return }
        Task {
            let authorized = await authorizeMicrophone()
            let allowed = fm_permission(trustedVoiceFrame(frame), type == .microphone, authorized)
            microphoneDecisions += 1
            traceVoice("microphone permission: \(allowed) key=\(window.isKeyWindow)")
            decisionHandler(allowed ? .grant : .deny)
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
