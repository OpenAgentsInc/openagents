// Firm reason: 6 (execute Coax-authored effects through the real native adapter).
import Foundation
import QuartzCore

// An SDK surface that reports its actual drawable unavailable. The real
// host must refuse before invoking the fixture producer or consuming time.
final class HostUnavailableLayer: CAMetalLayer {
    var attempts = 0
    override func nextDrawable() -> (any CAMetalDrawable)? { attempts += 1; return nil }
}

enum HostEffectError: Error { case failed(String) }
@main
struct HostEffects {
    @MainActor static func main() async {
        do { try await run() } catch { fputs("native host effects: \(error)\n", stderr); exit(1) }
    }
    @MainActor static func run() async throws {
        guard CommandLine.arguments.count == 5 else { throw HostEffectError.failed("HostEffects RESOURCES PRODUCER PLAN RESULT") }
        let plan = try JSONSerialization.jsonObject(with: Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[3]))) as? [String: Any] ?? [:]
        guard plan["schema"] as? String == "aethyr.host.effects.v1", let host = mf_open(CommandLine.arguments[1], CommandLine.arguments[2], false, true) else { throw HostEffectError.failed(mf_error(nil).map { String(cString: $0) } ?? "host open") }
        let network = ForgeMetalNetwork(host: host)
        defer { network.close(); mf_close(host) }
        mf_network_config(host, plan["origin"] as? String ?? "http://127.0.0.1:9170", "ws://127.0.0.1:9170")
        var result: [String: Any] = ["schema":"aethyr.host.observations.v1","host":"metal","scope":plan["scope"] as? String ?? "core"]
        var actions: [[Any]] = []
        func rows() -> [[Int64]] { (0..<mf_observe(host,-1,2)).map { event in (0..<8).map { mf_observe(host,event,Int64($0)) } } }
        func drain() {
            while let p = mf_take_effect(host) {
                if let e = try? JSONSerialization.jsonObject(with: Data(String(cString: p).utf8)) as? [String: Any] { network.execute(e) }
            }
            let code = mf_action(host)
            if code != 0 {
                var n = 0; let p = mf_outbox(host,&n)
                actions.append([code,p.map { String(decoding: UnsafeBufferPointer(start:$0,count:n),as:UTF8.self) } ?? ""])
                mf_action_done(host)
            }
        }
        network.changed = { drain() }
        for op in plan["operations"] as? [[String: Any]] ?? [] {
            func n(_ k: String) -> Int64 { (op[k] as? NSNumber)?.int64Value ?? 0 }
            func f(_ k: String) -> Float { (op[k] as? NSNumber)?.floatValue ?? 0 }
            switch op["op"] as? String {
            case "origin": mf_network_config(host,op["text"] as? String ?? "",nil)
            case "cookie":
                let properties: [HTTPCookiePropertyKey:Any] = [.domain:op["domain"] as? String ?? "",.path:op["path"] as? String ?? "/",.name:op["name"] as? String ?? "",.value:op["value"] as? String ?? "",.secure:op["secure"] as? Bool == true ? "TRUE":"FALSE"]
                guard let cookie = HTTPCookie(properties:properties), let jar = network.session.configuration.httpCookieStorage else { throw HostEffectError.failed("cookie fixture") }
                jar.setCookie(cookie)
            case "drawable-unavailable":
                let layer = HostUnavailableLayer(), before = mf_info(host,0), clock = mf_lifecycle_info(host,4)
                mf_layer(host,Unmanaged.passUnretained(layer).toOpaque())
                let accepted = mf_frame_at(host,320,180,1,1234)
                mf_layer(host,nil)
                guard accepted, layer.attempts == 1, mf_info(host,0) == before, mf_lifecycle_info(host,4) == clock else { throw HostEffectError.failed("unavailable drawable advanced producer or frame clock") }
                result["drawableRefusal"] = [layer.attempts,before,clock]
            case "asset":
                let data = Data((op["text"] as? String ?? "").utf8)
                _ = data.withUnsafeBytes { mf_asset(host,Int32(n("index")),$0.bindMemory(to:UInt8.self).baseAddress,$0.count) }
            case "reset": mf_observe_reset(host)
            case "frame": mf_still(host,op["still"] as? Bool ?? false); _ = mf_frame(host,f("width"),f("height"),f("dpr"),f("dt"))
            case "frame-at": _ = mf_frame_at(host,f("width"),f("height"),f("dpr"),n("us"))
            case "shown": mf_shown(host,op["value"] as? Bool ?? false)
            case "focus": mf_focus(host,op["value"] as? Bool ?? false)
            case "key": _ = mf_key(host,Int32(n("key")),op["down"] as? Bool ?? false,Int32(n("mods")))
            case "pointer": _ = mf_pointer(host,f("x"),f("y"),Int32(n("phase")),Int32(n("kind")),Int32(n("id")))
            case "wheel": mf_wheel(host,f("dy"))
            case "blur": mf_focus(host,false)
            case "message":
                let data = Data((op["text"] as? String ?? "").utf8)
                data.withUnsafeBytes { mf_message(host,Int32(n("kind")),$0.bindMemory(to:UInt8.self).baseAddress,$0.count) }
            case "wait": try await Task.sleep(for: .milliseconds(n("ms")))
            case "observe": result[op["name"] as? String ?? "events"] = rows()
            default: throw HostEffectError.failed("unknown SDK effect")
            }
            drain()
        }
        for (key,field) in [("version",0),("constructed",1),("dropped",3),("held",5)] { result[key] = mf_observe(host,-1,Int64(field)) }
        result["actions"] = actions
        result["lifecycle"] = (0..<5).map { mf_lifecycle_info(host,Int32($0)) }
        try JSONSerialization.data(withJSONObject:result,options:[.sortedKeys]).write(to:URL(fileURLWithPath:CommandLine.arguments[4]),options:.atomic)
    }
}
