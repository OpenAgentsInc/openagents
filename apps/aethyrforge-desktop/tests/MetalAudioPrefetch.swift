// Firm reason: 6 (actual AVFoundation decode, production adapter and immutable PCM observations).
import AVFoundation
import Foundation

@main
@MainActor
struct MetalAudioPrefetch {
    static func main() throws {
        guard CommandLine.arguments.count == 5 else { fatalError("MetalAudioPrefetch RESOURCES PRODUCER PLAN REPORT") }
        let resources=URL(fileURLWithPath:CommandLine.arguments[1]),report=URL(fileURLWithPath:CommandLine.arguments[4])
        let plan=try JSONSerialization.jsonObject(with:Data(contentsOf:URL(fileURLWithPath:CommandLine.arguments[3]))) as! [String:Any]
        if plan["phase"] as? String == "input" {
            let name="fire-hit-e2",file=try AVAudioFile(forReading:resources.appendingPathComponent("web/realms/everglade/sfx/\(name).ogg"))
            let pcm=AVAudioPCMBuffer(pcmFormat:file.processingFormat,frameCapacity:AVAudioFrameCount(file.length))!
            try file.read(into:pcm)
            let frames=Int(pcm.frameLength),channels=Int(pcm.format.channelCount)
            var samples=[Float](repeating:0,count:frames*channels)
            guard let planes=pcm.floatChannelData else { fatalError("SDK float decoder input") }
            for frame in 0..<frames { for channel in 0..<channels {
                samples[frame*channels+channel]=pcm.format.isInterleaved ? planes[0][frame*channels+channel]:planes[channel][frame]
            } }
            let output=report.deletingLastPathComponent().appendingPathComponent(name+"-input.f32")
            try samples.withUnsafeBytes { try Data($0).write(to:output,options:.atomic) }
            let observation: [String:Any]=["schema":"aethyr.metal.audio-prefetch-observed.v1","phase":"input","rows":[["name":name,"frames":frames,"channels":channels,"samples":samples.count,"rate":pcm.format.sampleRate,"file":output.path]],"closedWords":0,"deviceFrames":0]
            try JSONSerialization.data(withJSONObject:observation,options:[.sortedKeys,.prettyPrinted]).write(to:report,options:.atomic)
            return
        }
        guard plan["schema"] as? String == "aethyr.metal.audio-prefetch-plan.v1",
              let host=mf_open(resources.path,CommandLine.arguments[2],false,true) else { fatalError("effects-only producer") }
        defer { mf_close(host) }
        mf_network_config(host,"http://127.0.0.1:9","ws://127.0.0.1:9")
        let facts=Data("url=http://127.0.0.1:9/worlds/everglade?offline=1\n".utf8)
        facts.withUnsafeBytes { mf_message(host,600,$0.bindMemory(to:UInt8.self).baseAddress,$0.count) }
        let memory=ForgeAudioMemory(),audio=ForgeMetalAudio(host:host,resources:resources,muted:true,memory:memory,bankDirectory:report.deletingLastPathComponent().appendingPathComponent("bank"))
        defer { audio.close() }
        audio.bank()
        let registered=audio.bankWav.count,bankAssets=audio.buffers.count,bankWords=memory.total
        var rows: [[String:Any]]=[]
        if plan["phase"] as? String == "decode" {
            for name in plan["assets"] as? [String] ?? [] {
                var pcm=audio.buffer(name)
                var row: [String:Any]=["name":name,"samples":pcm?.count ?? 0,"channels":pcm?.channels ?? 0,"rate":48000,"memory":memory.total]
                let compressed=resources.appendingPathComponent("web/realms/everglade/sfx/\(name).ogg")
                let original=report.deletingLastPathComponent().appendingPathComponent(name+".ogg")
                try Data(contentsOf:compressed).write(to:original,options:.atomic)
                row["source"]=original.path
                if let decoded=pcm {
                    let output=report.deletingLastPathComponent().appendingPathComponent(name+".f32")
                    try Data(bytes:decoded.samples,count:decoded.count*4).write(to:output,options:.atomic)
                    row["file"]=output.path
                }
                rows.append(row);pcm=nil
            }
        } else {
            for frame in 0..<16 {
                guard mf_frame(host,320,180,1,1.0/60) else { fatalError("producer frame") }
                var n=0
                guard let p=mf_sound_commands(host,&n) else { continue }
                for line in Data(bytes:p,count:n).split(separator:10) {
                    let bytes=Data(line)
                    let (parsed,kind,name)=bytes.withUnsafeBytes { raw -> (Bool,Int64,String) in
                        var effect=MFAudioEffect()
                        let parsed=mf_audio_decode(raw.bindMemory(to:UInt8.self).baseAddress,raw.count,&effect)
                        let name=effect.name.map { String(decoding:UnsafeBufferPointer(start:$0,count:effect.name_count),as:UTF8.self) } ?? ""
                        return (parsed,effect.kind,name)
                    }
                    let result=audio.execute(bytes)
                    rows.append(["frame":frame,"line":String(decoding:bytes,as:UTF8.self),"parsed":parsed,"kind":kind,"name":name,"result":result,"samples":audio.buffers[name]?.count ?? 0,"channels":audio.buffers[name]?.channels ?? 0,"assets":audio.buffers.count,"memory":memory.total])
                }
            }
        }
        let deviceFrames=audio.source.observations()["deviceFrames",default:-1],held=memory.total,assets=audio.buffers.count,worldFrames=mf_info(host,0)
        audio.close()
        let output: [String:Any]=["schema":"aethyr.metal.audio-prefetch-observed.v1","phase":plan["phase"] as? String ?? "prefetch","registered":registered,"bankAssets":bankAssets,"bankWords":bankWords,"rows":rows,"heldAssets":assets,"heldWords":held,"closedWords":memory.total,"deviceFrames":deviceFrames,"worldFrames":worldFrames]
        try JSONSerialization.data(withJSONObject:output,options:[.sortedKeys,.prettyPrinted]).write(to:report,options:.atomic)
    }
}
