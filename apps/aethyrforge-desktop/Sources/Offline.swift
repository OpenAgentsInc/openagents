// Firm reason: 6 (native WebKit lifecycle and process measurement; gameplay stays in Coax).
import AppKit
import WebKit

@MainActor
extension ForgeApp {
    var offlineReportURL: URL? {
        guard let i=CommandLine.arguments.firstIndex(of:"--offline-check"),i+1<CommandLine.arguments.count else{return nil}
        return URL(fileURLWithPath:CommandLine.arguments[i+1])
    }
    var offlineWorker: Bool {CommandLine.arguments.contains("--offline-worker")}
    var offlineMode: Bool {offlineReportURL != nil || CommandLine.arguments.contains("--offline")}
    func offlineCheck() async throws {
        guard let report=offlineReportURL else{return}
        enterWorld()
        let deadline=Date().addingTimeInterval(30)
        var state:[String:Any]=[:]
        func snapshot() async throws -> [String:Any] {
            try await web.callAsyncJavaScript("return JSON.parse(JSON.stringify({url:location.href,host:coaxial.roomHost || {},workers:globalThis.__forgeRoomWorkers || 0,net:coaxial.netProbe(0),seat:coaxial.netProbe(1),loop:coaxial.worldLoop()}));",arguments:[:],in:nil,contentWorld:.page) as? [String:Any] ?? [:]
        }
        while Date()<deadline {
            state=try await snapshot()
            if let host=state["host"] as? [String:Any],host["native"] != nil,(state["net"] as? Int)==2,(state["seat"] as? Int)==0 {break}
            try await Task.sleep(for:.milliseconds(100))
        }
        guard let host=state["host"] as? [String:Any],(host["native"] as? Bool) == !offlineWorker,(state["net"] as? Int)==2,(state["seat"] as? Int)==0,
            (state["workers"] as? Int)==(offlineWorker ? 1:0) else {throw ShellError.failed("Wrong offline transport: \(state)")}
        let before=try await worldLoop(),start=ProcessInfo.processInfo.systemUptime
        let pid=webProcessPID(),roomPID=room?.processIdentifier ?? 0,appCPU=forge_process_cpu_ns(ProcessInfo.processInfo.processIdentifier),webCPU=forge_process_cpu_ns(pid),roomCPU=forge_process_cpu_ns(roomPID)
        var samples:[[String:Any]]=[]
        for _ in 0..<15 {
            try await Task.sleep(for:.seconds(1));samples.append(try await snapshot())
        }
        let after=try await worldLoop(),elapsed=ProcessInfo.processInfo.systemUptime-start
        let frames=(after["frames"] as? Int ?? 0)-(before["frames"] as? Int ?? 0)
        let messages=(after["socketFrames"] as? Int ?? 0)-(before["socketFrames"] as? Int ?? 0)
        guard frames>100,messages>100,let final=samples.last?["host"] as? [String:Any],
            (final["tickP50Us"] as? Double ?? 0)>0,(final["failed"] as? String ?? "").isEmpty else {
            throw ShellError.failed("Offline room did not simulate and deliver: \(samples.last ?? [:])")
        }
        let result:[String:Any]=["schema":"aethyrforge.offline.v1","transport":offlineWorker ? "wasm-worker":"native-room","firstFrameSeconds":firstFrameSeconds,
            "seconds":elapsed,"worldFrames":frames,"roomFramesReceived":messages,"workersCreated":state["workers"] ?? -1,
            "roomCPUPercentOfOneCore":Double(forge_process_cpu_ns(roomPID)-roomCPU)/1e9/elapsed*100,
            "appCPUPercentOfOneCore":Double(forge_process_cpu_ns(ProcessInfo.processInfo.processIdentifier)-appCPU)/1e9/elapsed*100,
            "webCPUPercentOfOneCore":Double(forge_process_cpu_ns(pid)-webCPU)/1e9/elapsed*100,"samples":samples]
        try JSONSerialization.data(withJSONObject:result,options:[.prettyPrinted,.sortedKeys]).write(to:report,options:.atomic)
        stopChildren();NSApp.terminate(nil)
    }
}
