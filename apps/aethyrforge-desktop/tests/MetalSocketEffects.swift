// Firm reason: 6 (execute a Coax-authored physical socket workload and retain raw SDK observations).
import Foundation

@main
struct MetalSocketEffects {
    static func main() throws {
        guard CommandLine.arguments.count == 3 else { fatalError("MetalSocketEffects PLAN REPORT") }
        let plan = try JSONSerialization.jsonObject(with:Data(contentsOf:URL(fileURLWithPath:CommandLine.arguments[1]))) as! [String:Any]
        var contexts: [String:UnsafeMutableRawPointer] = [:], held: [UnsafeMutableRawPointer] = []
        var observations: [[String:Any]] = []
        for op in plan["operations"] as! [[String:Any]] {
            let kind = op["op"] as! String, name = op["name"] as? String ?? "main"
            func n(_ key: String) -> Int64 { (op[key] as? NSNumber)?.int64Value ?? 0 }
            let owner = MNSocketOwner(id:n("id"),generation:n("generation"),engine:n("engine"))
            var result: [String:Any] = ["op":kind,"name":name,"label":op["label"] as? String ?? ""]
            switch kind {
            case "new":
                let context = mn_open(n("capacity")); contexts[name] = context; result["accepted"] = context != nil
            case "start":
                let bytes = Data((op["url"] as! String).utf8)
                result["accepted"] = bytes.withUnsafeBytes { mn_start(contexts[name],owner,$0.bindMemory(to:UInt8.self).baseAddress,$0.count) }
            case "send":
                let bytes = Data((op["text"] as! String).utf8)
                result["accepted"] = bytes.withUnsafeBytes { mn_send(contexts[name],owner,$0.bindMemory(to:UInt8.self).baseAddress,$0.count) }
            case "send-burst":
                let bytes = Data((op["text"] as! String).utf8)
                var accepted: [Bool] = [], maxima = [Int64](repeating:0,count:2)
                for _ in 0..<Int(n("count")) {
                    accepted.append(bytes.withUnsafeBytes { mn_send(contexts[name],owner,$0.bindMemory(to:UInt8.self).baseAddress,$0.count) })
                    for field in 0..<2 { maxima[field] = max(maxima[field],mn_info(contexts[name],Int32(field+2))) }
                }
                result["accepted"] = accepted; result["maxima"] = maxima
            case "stall": Thread.sleep(forTimeInterval:Double(n("ms"))/1000)
            case "info": result["fields"] = (0..<10).map { mn_info(contexts[name],Int32($0)) }; result["pool"] = (0..<5).map { mn_pool_info(Int32($0)) }
            case "drain":
                var events: [[String:Any]] = []
                for _ in 0..<Int(n("count")) {
                    var event = MNSocketEvent()
                    if !mn_next(contexts[name],&event) { break }
                    events.append(["owner":[event.owner.id,event.owner.generation,event.owner.engine],"event":event.event,"bytes":event.bytes.map { Array(UnsafeBufferPointer(start:$0,count:event.count)) } ?? []])
                    if op["hold"] as? Bool == true, let token = event.token { held.append(token) } else { mn_done(event.token) }
                }
                result["events"] = events
            case "drain-until-close":
                var events: [[String:Any]] = [], done = false
                let deadline = ProcessInfo.processInfo.systemUptime+Double(n("ms"))/1000
                while !done && events.count < Int(n("count")) && ProcessInfo.processInfo.systemUptime < deadline {
                    var event = MNSocketEvent()
                    if !mn_next(contexts[name],&event) { Thread.sleep(forTimeInterval:0.001); continue }
                    events.append(["owner":[event.owner.id,event.owner.generation,event.owner.engine],"event":event.event,"bytes":event.bytes.map { Array(UnsafeBufferPointer(start:$0,count:event.count)) } ?? []])
                    done = event.event == 2; mn_done(event.token)
                }
                result["events"] = events; result["closed"] = done
            case "close": if let context = contexts.removeValue(forKey:name) { mn_close(context) }
            case "release": for token in held { mn_done(token) }; held.removeAll()
            case "replace":
                var maxima = [Int64](repeating:0,count:5)
                let bytes = Data((op["url"] as! String).utf8)
                for _ in 0..<Int(n("count")) {
                    let context = mn_open(n("capacity"))!
                    _ = bytes.withUnsafeBytes { mn_start(context,owner,$0.bindMemory(to:UInt8.self).baseAddress,$0.count) }
                    for field in 0..<5 { maxima[field] = max(maxima[field],mn_pool_info(Int32(field))) }
                    mn_close(context)
                }
                result["maxima"] = maxima
            case "shutdown": mn_shutdown(); result["pool"] = (0..<5).map { mn_pool_info(Int32($0)) }
            default: fatalError("unknown SDK socket effect")
            }
            observations.append(result)
        }
        let report: [String:Any] = ["schema":"aethyr.metal.socket-effects.v1","observations":observations]
        try JSONSerialization.data(withJSONObject:report,options:[.sortedKeys,.prettyPrinted]).write(to:URL(fileURLWithPath:CommandLine.arguments[2]),options:.atomic)
    }
}
