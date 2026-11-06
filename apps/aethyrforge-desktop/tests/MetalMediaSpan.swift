// Firm reason: 6 (actual native reader/parser and producer-frame observations).
import Foundation
@main
@MainActor
struct MetalMediaSpan {
    static func main() throws {
        guard CommandLine.arguments.count == 5 else { fatalError("MetalMediaSpan RESOURCES SDK PLAN REPORT") }
        let plan=try JSONSerialization.jsonObject(with:Data(contentsOf:URL(fileURLWithPath:CommandLine.arguments[3]))) as! [String:Any]
        var host: UnsafeMutableRawPointer?, image="", rows: [Int64]=[], frames: Int64=0
        defer { if let host { mf_close(host) } }
        for step in plan["operations"] as! [[String:Any]] {
            let next=(step["capacity"] as! Bool) ? "libmediafixture.dylib":"libmediafixture-no-cap.dylib"
            if next != image {
                if let old=host { mf_close(old); host=nil }
                let path=URL(fileURLWithPath:CommandLine.arguments[2]).appendingPathComponent(next).path
                host=mf_open(CommandLine.arguments[1],path,false,true); image=next
            }
            guard let host else { fatalError("media fixture open") }
            var length=(step["length"] as! NSNumber).uint32Value
            withUnsafeBytes(of:&length){mf_message(host,(step["kind"] as! NSNumber).int32Value,$0.bindMemory(to:UInt8.self).baseAddress,$0.count)}
            var observed=[Int64](repeating:-1,count:5)
            guard mf_media_span_probe(host,&observed,observed.count) else { fatalError("actual media reader/parser") }
            rows += observed
            let before=mf_observe(host,-1,4)
            guard mf_frame(host,320,180,1,1.0/60), mf_observe(host,-1,4)==before+1 else { fatalError("producer continuation") }
            frames += 1
        }
        try JSONSerialization.data(withJSONObject:["schema":"aethyr.host.media-span.v1","host":"metal","observed":rows,"producerFrames":frames],options:[.sortedKeys]).write(to:URL(fileURLWithPath:CommandLine.arguments[4]),options:.atomic)
    }
}
