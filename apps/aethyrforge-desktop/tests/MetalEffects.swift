// Firm reason: 6 (windowless Metal SDK execution of a Coax-authored input plan).
import Foundation
import ImageIO

enum MetalEffectFailure: Error { case failed(String) }
func required<T>(_ v: T?, _ message: String) throws -> T { guard let v else { throw MetalEffectFailure.failed(message) }; return v }
func integer(_ x: Any?) -> Int { (x as? NSNumber)?.intValue ?? 0 }
func number(_ x: Any?) -> Float { (x as? NSNumber)?.floatValue ?? 0 }
func runEffects() throws {
    guard CommandLine.arguments.count == 5 else { throw MetalEffectFailure.failed("MetalEffects RESOURCES PRODUCER PLAN REPORT") }
    let data = try Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[3]))
    let plan = try required(JSONSerialization.jsonObject(with: data) as? [String: Any], "input plan")
    guard plan["schema"] as? String == "aethyr.metal.effects.v1" else { throw MetalEffectFailure.failed("input schema") }
    let host = try required(mf_open(CommandLine.arguments[1], CommandLine.arguments[2], plan["golden"] as? Bool ?? true, false), mf_error(nil).map { String(cString: $0) } ?? "native host failed")
    var open = true
    defer { if open { mf_close(host) } }
    func drain() throws {
        while let p = mf_take_effect(host) {
            let d = try required(JSONSerialization.jsonObject(with: Data(String(cString: p).utf8)) as? [String: Any], "effect")
            let id = Int64(integer(d["id"])), generation = (d["generation"] as? NSNumber)?.int64Value ?? 0, engine = (d["engine"] as? NSNumber)?.int64Value ?? 0
            if d["kind"] as? String == "http", id >= 0 {
                let bytes = Data("0\n".utf8)
                _ = mf_http_chunk(host, id, generation, engine, 2)
                bytes.withUnsafeBytes { mf_http_finish(host, id, generation, engine, $0.bindMemory(to: UInt8.self).baseAddress, $0.count, false, true) }
            } else if d["kind"] as? String == "socket-open" {
                mf_socket_event(host, id, generation, engine, 3, nil, 0)
                mf_socket_event(host, id, generation, engine, 2, nil, 0)
            } else if d["kind"] as? String == "socket-close" {
                mf_socket_event(host, id, generation, engine, 2, nil, 0)
            }
        }
    }
    var snapshots: [String] = []
    var images: [[String:Any]] = []
    var mediaOwnership: [[Int64]] = []
    let started = ProcessInfo.processInfo.systemUptime
    for op in plan["operations"] as? [[String: Any]] ?? [] {
        switch op["op"] as? String {
        case "bgra8": guard mf_offscreen_bgra8(host) else { throw MetalEffectFailure.failed("offscreen format must precede rendering") }
        case "presentation": guard mf_presentation_experiment(host) else { throw MetalEffectFailure.failed("presentation experiment must precede rendering") }
        case "presentation-v1": guard mf_presentation_experiment_v1(host) else { throw MetalEffectFailure.failed("private Coax v1 presentation descriptor is absent") }
        case "present-probe":
            let source = try Data(contentsOf:URL(fileURLWithPath:try required(op["file"] as? String,"presentation source")))
            let accepted = source.withUnsafeBytes { mf_present_probe(host,$0.bindMemory(to:UInt8.self).baseAddress,$0.count,Int64(integer(op["width"])),Int64(integer(op["height"])),Int64(integer(op["format"])),op["quantized"] as? Bool ?? false) }
            guard accepted else { throw MetalEffectFailure.failed(mf_error(host).map{String(cString:$0)} ?? "presentation probe refused") }
        case "presented":
            let path = try required(op["file"] as? String,"presented snapshot")
            guard mf_snapshot_presented(host,path) else { throw MetalEffectFailure.failed("actual UNORM presentation readback") }
            snapshots.append(path)
        case "presented-raw":
            let path = try required(op["file"] as? String,"raw display snapshot")
            guard mf_snapshot_presented_raw(host,path) else { throw MetalEffectFailure.failed("raw UNORM display readback") }
            snapshots.append(path)
        case "media-raw":
            let path = try required(op["file"] as? String,"media snapshot"), url = try required(op["url"] as? String,"media URL")
            guard mf_media_snapshot(host,url,path) else { throw MetalEffectFailure.failed("actual ready media texture readback") }
            snapshots.append(path)
        case "media-notice":
            var count = 0
            let p = try required(mf_media_notice(host,&count),"delivered media notice")
            try Data(bytes:p,count:count).write(to:URL(fileURLWithPath:try required(op["file"] as? String,"media notice path")),options:.atomic)
        case "media-state":
            let rows = (op["urls"] as? [String] ?? []).map { url in (0..<5).map { mf_media_info(host,url,Int32($0)) } }
            try JSONSerialization.data(withJSONObject:rows).write(to:URL(fileURLWithPath:try required(op["file"] as? String,"media state path")),options:.atomic)
        case "media-ownership":
            mediaOwnership.append((0..<2).map { mf_media_cache_info(host,Int32($0)) })
        case "replay":
            let path = try required(op["file"] as? String, "frame config")
            guard mf_replay(host,path) else { throw MetalEffectFailure.failed(mf_error(host).map { String(cString:$0) } ?? "frozen frame refused") }
        case "recover":
            guard mf_recover(host) else { throw MetalEffectFailure.failed(mf_error(host).map { String(cString:$0) } ?? "resource recovery refused") }
        case "dump":
            let directory = URL(fileURLWithPath:try required(op["file"] as? String,"CPU dump directory"))
            try FileManager.default.createDirectory(at:directory,withIntermediateDirectories:true)
            for (kind,name) in ["u.bin","hud.bin","draws.bin","s0.bin","s1.bin","s2.bin","s3.bin","puffs.bin","state.bin","media.txt"].enumerated() {
                var count = 0
                guard let bytes = mf_cpu_bytes(host,Int32(kind),&count) else { throw MetalEffectFailure.failed("missing observed CPU allocation") }
                try Data(bytes:bytes,count:count).write(to:directory.appendingPathComponent(name),options:.atomic)
            }
            let fields = (0..<128).map { mf_probe(host,Int32($0)) }
            try JSONSerialization.data(withJSONObject:fields).write(to:directory.appendingPathComponent("fields.json"),options:.atomic)
            var capacities: [String:Int] = [:]
            for (kind,name) in [(0,"u.bin"),(3,"s0.bin"),(4,"s1.bin"),(5,"s2.bin"),(6,"s3.bin"),(7,"puffs.bin")] { capacities[name] = mf_cpu_capacity(host,Int32(kind)) }
            try JSONSerialization.data(withJSONObject:capacities,options:.sortedKeys).write(to:directory.appendingPathComponent("capacities.json"),options:.atomic)
        case "message":
            let bytes = Data((op["text"] as? String ?? "").utf8)
            bytes.withUnsafeBytes { mf_message(host, Int32(integer(op["kind"])), $0.bindMemory(to: UInt8.self).baseAddress, $0.count) }
        case "key": _ = mf_key(host, Int32(integer(op["key"])), (op["down"] as? Bool) == true, Int32(integer(op["mods"])))
        case "pointer": _ = mf_pointer(host, number(op["x"]), number(op["y"]), Int32(integer(op["phase"])), Int32(integer(op["kind"])), Int32(integer(op["id"])))
        case "frame":
            let before = mf_info(host, 0), deadline = ProcessInfo.processInfo.systemUptime + 10
            repeat {
                guard mf_frame(host, number(op["width"]), number(op["height"]), number(op["scale"]), number(op["dt"])) else { throw MetalEffectFailure.failed(mf_error(host).map { String(cString: $0) } ?? "frame refused") }
                RunLoop.main.run(until: Date().addingTimeInterval(0.001))
            } while mf_info(host, 0) == before && ProcessInfo.processInfo.systemUptime < deadline
            guard mf_info(host, 0) > before else { throw MetalEffectFailure.failed("frame completion deadline") }
        case "snapshot":
            let path = try required(op["file"] as? String, "snapshot file")
            guard mf_snapshot(host, path) else { throw MetalEffectFailure.failed("snapshot effect") }
            snapshots.append(path)
        case "jpeg":
            var count = 0
            guard let bytes = mf_frame_jpeg(host,&count), let jpeg = Data(base64Encoded:Data(bytes:bytes,count:count)),
                  let image = CGImageSourceCreateWithData(jpeg as CFData,nil), let props = CGImageSourceCopyPropertiesAtIndex(image,0,nil) as? [CFString:Any] else { throw MetalEffectFailure.failed("actual frame JPEG") }
            let input = Data((op["body"] as? String ?? "{}").utf8), encoded = Data(bytes:bytes,count:count)
            var attached = Data(count:input.count+encoded.count+64)
            let length = attached.withUnsafeMutableBytes { out in input.withUnsafeBytes { body in encoded.withUnsafeBytes { b64 in
                mf_http_image(body.bindMemory(to:UInt8.self).baseAddress,body.count,b64.bindMemory(to:UInt8.self).baseAddress,b64.count,out.bindMemory(to:UInt8.self).baseAddress,out.count)
            } } }
            guard length >= 0 else { throw MetalEffectFailure.failed("Coax image attachment") }
            let path = op["file"] as! String
            try jpeg.write(to:URL(fileURLWithPath:path+".jpg"),options:.atomic)
            try attached.prefix(Int(length)).write(to:URL(fileURLWithPath:path+".json"),options:.atomic)
            images.append(["width":props[kCGImagePropertyPixelWidth] as? NSNumber ?? 0,"height":props[kCGImagePropertyPixelHeight] as? NSNumber ?? 0,"bytes":jpeg.count,"file":path])
        default: throw MetalEffectFailure.failed("unknown admitted SDK operation")
        }
        try drain()
    }
    let deadline = ProcessInfo.processInfo.systemUptime + 5
    while mf_info(host,2) != 0 && ProcessInfo.processInfo.systemUptime < deadline { RunLoop.main.run(until: Date().addingTimeInterval(0.001)) }
    var result: [String: Any] = ["schema": "aethyr.metal.effects-result.v1", "frames": mf_info(host, 0), "completed":mf_info(host,10), "draws": mf_info(host, 1), "pending": mf_info(host, 2), "ledgerLive": mf_info(host, 3), "ledgerWords": mf_info(host, 4), "ledgerEpoch": mf_info(host, 5), "stateAddress": mf_info(host, 6), "seconds": ProcessInfo.processInfo.systemUptime - started, "snapshots": snapshots]
    result["recoveries"] = mf_info(host,12); result["recoveryReleasedLive"] = mf_info(host,13)
    result["images"] = images
    result["mediaOwnership"] = mediaOwnership
    mf_close(host); open = false
    result["closedLedgerLive"] = mf_closed_info(0); result["closedLedgerWords"] = mf_closed_info(1)
    result["closedMediaBytes"] = mf_closed_info(2)
    try JSONSerialization.data(withJSONObject: result, options: [.prettyPrinted, .sortedKeys]).write(to: URL(fileURLWithPath: CommandLine.arguments[4]), options: .atomic)
}
do { try runEffects() } catch { fputs("Metal SDK effects: \(error)\n", stderr); exit(1) }
