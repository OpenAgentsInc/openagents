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
        if ["decode","picked","authored","canonical"].contains(plan["phase"] as? String ?? "") {
            for name in plan["assets"] as? [String] ?? [] {
                // Whole-bank codec observation owns one decoded asset at a
                // time, independently of the production demand-cache receipt.
                let picked=plan["phase"] as? String != "decode"
                var pcm=plan["phase"] as? String == "canonical" ? audio.loadAsset(name):(picked ? audio.decode(resources.appendingPathComponent("web/realms/everglade/sfx/\(name).ogg")):audio.buffer(name))
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
                pcm=nil;row["releasedWords"]=memory.total;rows.append(row)
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
        var faults: [Int64]=[]
        if plan["phase"] as? String == "canonical" {
            let root=report.deletingLastPathComponent().appendingPathComponent("refusal-resources"),bank=root.appendingPathComponent("web/realms/everglade"),sfx=bank.appendingPathComponent("sfx"),companions=bank.appendingPathComponent("pcm-v1")
            try FileManager.default.createDirectory(at:sfx,withIntermediateDirectories:true)
            try FileManager.default.createDirectory(at:companions,withIntermediateDirectories:true)
            let hit=try Data(contentsOf:resources.appendingPathComponent("web/realms/everglade/sfx/fire-hit-e2.ogg")),cast=try Data(contentsOf:resources.appendingPathComponent("web/realms/everglade/sfx/fire-cast-e2.ogg")),companion=try Data(contentsOf:resources.appendingPathComponent("web/realms/everglade/pcm-v1/fire-hit-e2.apc"))
            try hit.write(to:sfx.appendingPathComponent("fire-hit-e2.ogg"));try cast.write(to:sfx.appendingPathComponent("fire-cast-e2.ogg"));try cast.write(to:sfx.appendingPathComponent("probe-safe.ogg"))
            let owner=ForgeMetalAudio(host:host,resources:root,muted:true)
            func observe(_ name: String) {
                let sdk=owner.sdkDecodes,canonical=owner.companionDecodes
                var pcm=owner.loadAsset(name);faults.append(Int64(pcm?.count ?? 0));faults.append(Int64(owner.sdkDecodes-sdk));faults.append(Int64(owner.companionDecodes-canonical));pcm=nil;faults.append(owner.memory.total)
            }
            observe("fire-hit-e2") // Missing required companion.
            var damaged=companion;damaged[damaged.count-1] ^= 1;try damaged.write(to:companions.appendingPathComponent("fire-hit-e2.apc"));observe("fire-hit-e2")
            try companion.write(to:companions.appendingPathComponent("fire-hit-e2.apc"));damaged=hit;damaged[damaged.count-1] ^= 1;try damaged.write(to:sfx.appendingPathComponent("fire-hit-e2.ogg"));observe("fire-hit-e2")
            try hit.write(to:sfx.appendingPathComponent("fire-hit-e2.ogg"));observe("fire-hit-e2");observe("fire-cast-e2");observe("probe-safe")
            var ticket=owner.memory.reserve(frames:8388608,channels:1)
            let sdk=owner.sdkDecodes,canonical=owner.companionDecodes
            var refused=withExtendedLifetime(ticket) { owner.loadAsset("fire-hit-e2") };faults.append(Int64(refused?.count ?? 0));faults.append(Int64(owner.sdkDecodes-sdk));faults.append(Int64(owner.companionDecodes-canonical));refused=nil;ticket=nil;faults.append(owner.memory.total)
            owner.close()
        }
        let output: [String:Any]=["schema":"aethyr.metal.audio-prefetch-observed.v1","phase":plan["phase"] as? String ?? "prefetch","registered":registered,"bankAssets":bankAssets,"bankWords":bankWords,"rows":rows,"heldAssets":assets,"heldWords":held,"closedWords":memory.total,"deviceFrames":deviceFrames,"worldFrames":worldFrames,"sdkDecodes":audio.sdkDecodes,"companionDecodes":audio.companionDecodes,"refusal":faults]
        try JSONSerialization.data(withJSONObject:output,options:[.sortedKeys,.prettyPrinted]).write(to:report,options:.atomic)
    }
}
