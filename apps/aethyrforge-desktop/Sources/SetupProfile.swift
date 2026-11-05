// Firm reason: 6 (AppKit/WebKit/Metal/IOKit measurements and native image readback).
import AppKit
import WebKit
import MetalKit
import IOKit

@MainActor
extension ForgeApp {
    var setupProfileURL: URL? {
        guard let i = CommandLine.arguments.firstIndex(of: "--setup-profile"), i+1 < CommandLine.arguments.count else { return nil }
        return URL(fileURLWithPath: CommandLine.arguments[i+1])
    }
    func gpuUtilization() -> Double? {
        let service = IOServiceGetMatchingService(kIOMainPortDefault, IOServiceMatching("AGXAccelerator"))
        guard service != 0 else { return nil }; defer { IOObjectRelease(service) }
        guard let stats = IORegistryEntryCreateCFProperty(service,"PerformanceStatistics" as CFString,kCFAllocatorDefault,0)?.takeRetainedValue() as? [String:Any] else { return nil }
        return (stats["Device Utilization %"] as? NSNumber)?.doubleValue
    }
    func webProcessPID() -> Int32 {
        let selector = NSSelectorFromString("_webProcessIdentifier")
        guard web.responds(to: selector) else { return 0 }
        let get = unsafeBitCast(web.method(for: selector),to:(@convention(c)(AnyObject,Selector)->Int32).self)
        return get(web,selector)
    }
    func worldLoop() async throws -> [String:Any] {
        try await web.callAsyncJavaScript("return coaxial.worldLoop();",arguments:[:],in:nil,contentWorld:.page) as? [String:Any] ?? [:]
    }
    func capturePanel(_ path: URL) throws {
        let size = setupPane.drawableSize, width = Int(size.width), height = Int(size.height)
        guard width>0,height>0,let device=setupPane.device else { throw ShellError.failed("Panel has no drawable size.") }
        let descriptor=MTLTextureDescriptor.texture2DDescriptor(pixelFormat:.bgra8Unorm,width:width,height:height,mipmapped:false)
        descriptor.usage=[.renderTarget,.shaderRead];descriptor.storageMode = .shared
        let texture=device.makeTexture(descriptor:descriptor)!,pass=MTLRenderPassDescriptor()
        pass.colorAttachments[0].texture=texture;pass.colorAttachments[0].storeAction = .store
        var command:MTLCommandBuffer?
        setupPane.render(pass,size:size,dpr:window.backingScaleFactor) {command=$0}
        command?.waitUntilCompleted()
        guard command?.status == .completed else {throw ShellError.failed("Panel readback failed.")}
        var bytes=[UInt8](repeating:0,count:width*height*4)
        texture.getBytes(&bytes,bytesPerRow:width*4,from:MTLRegionMake2D(0,0,width,height),mipmapLevel:0)
        let provider=CGDataProvider(data:Data(bytes) as CFData)!
        let image=CGImage(width:width,height:height,bitsPerComponent:8,bitsPerPixel:32,bytesPerRow:width*4,
            space:CGColorSpaceCreateDeviceRGB(),bitmapInfo:CGBitmapInfo(rawValue:CGImageAlphaInfo.premultipliedFirst.rawValue).union(.byteOrder32Little),provider:provider,decode:nil,shouldInterpolate:false,intent:.defaultIntent)!
        try NSBitmapImageRep(cgImage:image).representation(using:.png,properties:[:])!.write(to:path)
    }
    func setupProfileCheck() async throws {
        guard let report=setupProfileURL,sceneReady,fgs_can_enter(sceneReady) else {throw ShellError.failed("Setup profiling needs the real first frame and ready discovery.")}
        let webPID=webProcessPID()
        guard webPID>0 else {throw ShellError.failed("Cannot measure the WKWebView process CPU.")}
        var rows:[[String:Any]]=[]
        func sample(_ name:String,_ expectPaused:Bool) async throws {
            try await Task.sleep(for:.milliseconds(1200))
            let before=try await worldLoop(),start=ProcessInfo.processInfo.systemUptime
            let appCPU=forge_process_cpu_ns(ProcessInfo.processInfo.processIdentifier),webCPU=forge_process_cpu_ns(webPID)
            let draws=setupPane.drawCount,gpu=setupPane.gpuSeconds
            var utilization:[Double]=[]
            for _ in 0..<40 {if let v=gpuUtilization(){utilization.append(v)};try await Task.sleep(for:.milliseconds(200))}
            let elapsed=ProcessInfo.processInfo.systemUptime-start,after=try await worldLoop()
            func delta(_ key:String)->Int {((after[key] as? NSNumber)?.intValue ?? 0)-((before[key] as? NSNumber)?.intValue ?? 0)}
            let frames=delta("frames"),submissions=delta("submissions"),messages=delta("socketFrames")
            let pending=(after["pending"] as? NSNumber)?.intValue ?? -1
            guard (after["paused"] as? Bool)==expectPaused,expectPaused ? frames==0 && submissions==0 && pending==0 : frames>0 && submissions>0,messages>0 else {
                throw ShellError.failed("World/room visibility regression in \(name): \(before), \(after)")
            }
            let appPercent=Double(forge_process_cpu_ns(ProcessInfo.processInfo.processIdentifier)-appCPU)/1e9/elapsed*100
            let webPercent=Double(forge_process_cpu_ns(webPID)-webCPU)/1e9/elapsed*100
            utilization.sort()
            var row:[String:Any]=["phase":name,"seconds":elapsed,"paused":expectPaused,"worldFrames":frames,"worldSubmissions":submissions,"roomFramesReceived":messages,
                "appCPUPercentOfOneCore":appPercent,"webCPUPercentOfOneCore":webPercent,"panelDraws":setupPane.drawCount-draws,"panelGPUSeconds":setupPane.gpuSeconds-gpu,"worldPendingGPUFrames":pending]
            if !utilization.isEmpty {row["systemGPUUtilizationP50Percent"]=utilization[utilization.count/2];row["systemGPUUtilizationMaxPercent"]=utilization.last!}
            rows.append(row)
            try JSONSerialization.data(withJSONObject:["schema":"aethyrforge.setup-profile.v1","firstFrameSeconds":firstFrameSeconds,"webPID":webPID,"rows":rows],options:[.prettyPrinted,.sortedKeys]).write(to:report,options:.atomic)
            if name=="setup" || name=="status" {
                guard setupPane.lastVertexCount>0,setupPane.drawCount>0,!setupPane.subviews.contains(where:{$0 is NSTextField || $0 is NSVisualEffectView}) else {throw ShellError.failed("Panel is not rendered by shared Zeron Metal.")}
                try capturePanel(report.deletingLastPathComponent().appendingPathComponent(name+".png"))
            }
        }
        enterWorld();try await sample("play",false)
        showSetup();try await sample("setup",true)
        showStatus();try await sample("status",true)
        reloadWorld()
        let reloaded=Date().addingTimeInterval(20)
        while !sceneReady && Date()<reloaded {try await Task.sleep(for:.milliseconds(100))}
        guard sceneReady else {
            let debug=try? await web.evaluateJavaScript("JSON.stringify({url:location.href,ready:coaxial?.ready,scene:coaxial?.sceneReady,loop:coaxial?.worldLoop?.(),render:coaxial?.renderStatus,errors:globalThis.__forgeErrors,loading:document.querySelector('#coaxial-load')?.textContent})")
            throw ShellError.failed("Setup blocked the first frame after reload: \(String(describing:debug))")
        }
        try await sample("statusReloaded",true)
        enterWorld();window.miniaturize(nil);try await sample("minimized",true)
        window.deminiaturize(nil);window.makeKeyAndOrderFront(nil);NSApp.activate(ignoringOtherApps:true)
        try await sample("resumed",false)
        NSApp.hide(nil);try await sample("hidden",true)
        NSApp.unhide(nil);window.makeKeyAndOrderFront(nil);NSApp.activate(ignoringOtherApps:true)
        try await sample("unhidden",false)
        stopChildren();NSApp.terminate(nil)
    }
}
