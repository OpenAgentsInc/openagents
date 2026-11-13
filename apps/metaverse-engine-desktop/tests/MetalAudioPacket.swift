// Firm reason: 6 (actual native packet-pointer/length observations, no audio device).
import Foundation
@main
@MainActor
struct MetalAudioPacket {
    static func main() throws {
        guard CommandLine.arguments.count == 5,
              let host=mf_open(CommandLine.arguments[1],CommandLine.arguments[2],false,true) else { fatalError("packet fixture open") }
        defer { mf_close(host) }
        let plan=try JSONSerialization.jsonObject(with:Data(contentsOf:URL(fileURLWithPath:CommandLine.arguments[3]))) as! [String:Any]
        var rows: [Int64]=[], packets: [String]=[]
        let capacity=(plan["copyCapacity"] as? NSNumber)?.intValue ?? 4
        for step in plan["operations"] as! [[String:Any]] {
            if step["frame"] as? Bool == true { guard mf_frame(host,320,180,1,1.0/60) else { fatalError("packet producer frame") } }
            if let kind=step["kind"] as? NSNumber {
                var length=(step["length"] as! NSNumber).uint32Value
                withUnsafeBytes(of:&length) { mf_message(host,kind.int32Value,$0.bindMemory(to:UInt8.self).baseAddress,$0.count) }
            }
            var count=0
            let packet=mf_sound_commands(host,&count)
            // Never dereference a refused or physically oversized observation.
            let data=packet != nil && count>0 && count<=capacity ? Data(bytes:packet!,count:count):Data()
            let sum=data.reduce(Int64(0)){$0+Int64($1)}
            packets.append(String(decoding:data,as:UTF8.self))
            rows += [packet == nil ? 0:1,Int64(count),sum]
        }
        try JSONSerialization.data(withJSONObject:["schema":"aethyr.host.audio-packet.v1","host":"metal","observations":rows,"packets":packets],options:[.sortedKeys]).write(to:URL(fileURLWithPath:CommandLine.arguments[4]),options:.atomic)
    }
}
