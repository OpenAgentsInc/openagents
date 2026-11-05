// Firm reason: 6 (visible WebKit lifecycle, SDK process measurements and benchmark byte transport).
import AppKit
import WebKit

@MainActor
extension ForgeApp {
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
