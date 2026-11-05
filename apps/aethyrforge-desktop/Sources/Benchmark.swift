// Firm reason: 6 (visible WebKit lifecycle, SDK process measurements and benchmark byte transport).
import AppKit
import WebKit
import QuartzCore

@MainActor
extension ForgeApp {
    // Execute only SDK effects from the Coax-authored native measurement
    // plan. Coax reduces these raw clocks and decides the qualification.
    func measureMetal(_ planURL: URL) async throws {
        guard let view = metalWorld, let host = view.host,
              let plan = try JSONSerialization.jsonObject(with:Data(contentsOf:planURL)) as? [String:Any],
              plan["schema"] as? String == "aethyr.metal.window-plan.v1",
              let report = plan["report"] as? String else { throw ShellError.failed("Invalid native measurement plan.") }
        var observations: [[String:Any]] = []
        let started = ProcessInfo.processInfo.systemUptime
        let cpu = forge_process_cpu_ns(ProcessInfo.processInfo.processIdentifier)
        for op in plan["operations"] as? [[String:Any]] ?? [] {
            guard !stopping, view.host == host else {
                observations.append(["label":"host-stopped","error":notice.stringValue,"stopping":stopping,"sameHost":view.host == host])
                break
            }
            switch op["op"] as? String {
            case "wait": try await Task.sleep(for:.milliseconds((op["ms"] as! NSNumber).intValue))
            case "raise": NSApp.unhide(nil); window.deminiaturize(nil); window.makeKeyAndOrderFront(nil); NSApp.activate(ignoringOtherApps:true)
            case "reset": mf_measure_reset(host)
            case "audio": view.audio?.start()
            case "presentation":
                view.preferredHz = (op["hz"] as! NSNumber).floatValue
                view.display?.preferredFrameRateRange = CAFrameRateRange(minimum:view.preferredHz,maximum:view.preferredHz,preferred:view.preferredHz)
            case "key": _ = mf_key(host,(op["key"] as! NSNumber).int32Value,op["down"] as? Bool ?? false,0)
            case "status": showStatus()
            case "world": enterWorld()
            case "focus": view.focus(op["focused"] as? Bool ?? false)
            case "resize": window.setContentSize(NSSize(width:(op["width"] as! NSNumber).doubleValue,height:(op["height"] as! NSNumber).doubleValue))
            case "snapshot": guard mf_snapshot(host,op["file"] as! String) else { throw ShellError.failed("Native snapshot failed.") }
            case "presented": guard mf_snapshot_presented(host,op["file"] as! String) else { throw ShellError.failed("Native display readback failed.") }
            case "jpeg":
                var count = 0
                guard let bytes = mf_frame_jpeg(host,&count),let image = Data(base64Encoded:Data(bytes:bytes,count:count)),!image.isEmpty else { throw ShellError.failed("Native display JPEG failed.") }
                try image.write(to:URL(fileURLWithPath:op["file"] as! String),options:.atomic)
            case "observe":
                var count = 0, dropped = 0
                let records = mf_measure_rows(host,&count,&dropped)
                let rows = records.map { p in (0..<count).map { i -> [Any] in
                    let r = p[i]
                    return [r.frame,r.start_ns,r.submit_ns,r.complete_ns,r.gpu_ns,r.input_sequence,r.input_ns,r.x_mm,r.z_mm,r.width,r.height,r.scale]
                } } ?? []
                observations.append(["label":op["label"] as? String ?? "", "rows":rows,"dropped":dropped,"frames":mf_info(host,0),"completed":mf_info(host,10),"pending":mf_info(host,2),"net":mf_info(host,7),"seat":mf_info(host,8),"messages":view.network?.delivered ?? 0,"paused":view.paused,"setup":!setupPane.isHidden,"ledgerLive":mf_info(host,3),"recoveries":mf_info(host,12),"width":view.bounds.width,"height":view.bounds.height,"backingScale":window.backingScaleFactor,"audio":view.audio?.source.observations() ?? [:],"nativeFace":(0..<11).map { mf_observe(host,-2,Int64($0)) },"windowVisible":window.isVisible,"occlusionVisible":window.occlusionState.contains(.visible),"onActiveSpace":window.isOnActiveSpace,"appHidden":NSApp.isHidden,"appActive":NSApp.isActive])
            default: throw ShellError.failed("Unknown native measurement effect.")
            }
        }
        let seconds = ProcessInfo.processInfo.systemUptime-started
        var result: [String:Any] = ["schema":"aethyr.metal.window-result.v1","observations":observations,"seconds":seconds,"cpuPercentOfOneCore":Double(forge_process_cpu_ns(ProcessInfo.processInfo.processIdentifier)-cpu)/1e9/seconds*100,"residentBytes":forge_process_resident_bytes(ProcessInfo.processInfo.processIdentifier),"displayFPS":window.screen?.maximumFramesPerSecond ?? 0,"preferredFPS":view.preferredHz,"visible":window.isVisible && window.occlusionState.contains(.visible),"thermal":ProcessInfo.processInfo.thermalState.rawValue,"lowPower":ProcessInfo.processInfo.isLowPowerModeEnabled,"firstFrameSeconds":firstFrameSeconds]
        let closedAudio = view.audio?.source
        view.close(); result["closedLedgerLive"] = mf_closed_info(0); result["closedLedgerWords"] = mf_closed_info(1)
        result["closedAudio"] = closedAudio?.observations() ?? [:]
        try JSONSerialization.data(withJSONObject:result,options:[.prettyPrinted,.sortedKeys]).write(to:URL(fileURLWithPath:report),options:.atomic)
        stopChildren(); NSApp.terminate(nil)
    }
    var benchmarkReportURL: URL? {CommandLine.arguments.contains("--frame-benchmark") ? frameProfileURL:nil}
    func benchmarkGPUPID()->Int32 {
        // WebKit's guarded diagnostic PID property; no behavior depends on it.
        let selector=NSSelectorFromString("_gpuProcessIdentifier");guard web.responds(to:selector) else{return 0}
        let get=unsafeBitCast(web.method(for:selector),to:(@convention(c)(AnyObject,Selector)->Int32).self);return get(web,selector)
    }
    func renderBenchmark(_ report:URL) async throws {
        enterWorld()
        let probeOnly=CommandLine.arguments.contains("--bench-probe"),warmup=CommandLine.arguments.contains("--bench-probe") ? 2:40
        let deadline=Date().addingTimeInterval(60)
        while Date()<deadline {
            let ready=try await web.evaluateJavaScript("if(coaxial.netProbe(0)!==2){dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',bubbles:true}));dispatchEvent(new KeyboardEvent('keyup',{key:'Enter',bubbles:true}));} Math.floor(__evergladeBenchmark.replaySeconds())") as? Int ?? 0
            if ready>=warmup {break};try await Task.sleep(for:.milliseconds(200))
        }
        guard try await web.evaluateJavaScript("coaxial.netProbe(0)===2 && __evergladeBenchmark.replaySeconds()>=\(warmup)") as? Bool == true else {throw ShellError.failed("Benchmark replay did not connect.")}
        _=try await web.evaluateJavaScript("__evergladeBenchmark.snapshot();true")
        let pid=webProcessPID(),gpuPID=benchmarkGPUPID(),began=ProcessInfo.processInfo.systemUptime
        let appCPU=forge_process_cpu_ns(ProcessInfo.processInfo.processIdentifier),webCPU=forge_process_cpu_ns(pid)
        let gpuCPU=forge_process_cpu_ns(gpuPID)
        var windows:[[String:Any]]=[]
        for _ in 0..<(probeOnly ? 1:6) {
            _=try await web.evaluateJavaScript("__evergladeBenchmark.tap();true")
            try await Task.sleep(for:.seconds(probeOnly ? 1:5))
            let raw=try await web.evaluateJavaScript("JSON.stringify(__evergladeBenchmark.snapshot())") as? String ?? "{}"
            var window=try JSONSerialization.jsonObject(with:Data(raw.utf8)) as? [String:Any] ?? [:]
            window["visible"]=self.window.isVisible && self.window.occlusionState.contains(.visible)
            window["webResidentBytes"]=forge_process_resident_bytes(pid)
            window["gpuResidentBytes"]=forge_process_resident_bytes(gpuPID)
            window["thermal"]=ProcessInfo.processInfo.thermalState.rawValue
            window["lowPower"]=ProcessInfo.processInfo.isLowPowerModeEnabled
            windows.append(window)
        }
        let seconds=ProcessInfo.processInfo.systemUptime-began
        let result:[String:Any]=["schema":"everglade.benchmark.v1","host":"WKWebView","windows":windows,"firstFrameSeconds":firstFrameSeconds,"seconds":seconds,
            "appCPUPercentOfOneCore":Double(forge_process_cpu_ns(ProcessInfo.processInfo.processIdentifier)-appCPU)/1e9/seconds*100,
            "webCPUPercentOfOneCore":Double(forge_process_cpu_ns(pid)-webCPU)/1e9/seconds*100,"appResidentBytes":forge_process_resident_bytes(ProcessInfo.processInfo.processIdentifier),
            "webResidentBytes":forge_process_resident_bytes(pid),"gpuPID":gpuPID,"gpuResidentBytes":forge_process_resident_bytes(gpuPID),
            "gpuProcessCPUPercentOfOneCore":Double(forge_process_cpu_ns(gpuPID)-gpuCPU)/1e9/seconds*100,"displayFPS":self.window.screen?.maximumFramesPerSecond ?? 0]
        try JSONSerialization.data(withJSONObject:result,options:[.prettyPrinted,.sortedKeys]).write(to:report,options:.atomic)
        try await captureCanvas(report)
        stopChildren();NSApp.terminate(nil)
    }
}
