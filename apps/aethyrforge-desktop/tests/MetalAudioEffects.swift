// Firm reason: 6 (muted CoreAudio and decoded-PCM effects from a Coax-authored plan).
import AVFoundation
import Foundation

@main
@MainActor
struct MetalAudioEffects {
    static func main() throws {
        guard CommandLine.arguments.count == 5 else { fatalError("MetalAudioEffects RESOURCES PRODUCER PLAN REPORT") }
        let res = URL(fileURLWithPath:CommandLine.arguments[1])
        let plan = try JSONSerialization.jsonObject(with:Data(contentsOf:URL(fileURLWithPath:CommandLine.arguments[3]))) as! [String:Any]
        guard plan["schema"] as? String == "aethyr.metal.audio-effects.v1", let host = mf_open(res.path,CommandLine.arguments[2],true,false) else { fatalError("audio producer open") }
        var audio = ForgeMetalAudio(host:host,resources:res,muted:true)
        defer { audio.close(); mf_close(host) }
        var events: [[String:Any]] = []
        for op in plan["operations"] as? [[String:Any]] ?? [] {
            let kind = op["op"] as? String ?? ""
            switch kind {
            case "frame":
                var lines = Data(), results: [Int64] = []
                for _ in 0..<((op["count"] as? NSNumber)?.intValue ?? 1) {
                    guard mf_frame(host,320,180,1,1.0/60) else { fatalError("audio frame") }
                    RunLoop.main.run(until:Date().addingTimeInterval(0.005))
                    if op["capture"] as? Bool == true {
                        var count = 0
                        if let p = mf_sound_commands(host,&count), count > 0 {
                            let data = Data(bytes:p,count:count); lines.append(data)
                            for line in data.split(separator:10) { results.append(audio.execute(Data(line),contextStarted:true)) }
                        }
                    }
                }
                if op["capture"] as? Bool == true { events.append(["op":kind,"text":String(decoding:lines,as:UTF8.self),"results":results]) }
            case "bank":
                audio.bank()
                events.append(["op":kind,"assets":audio.buffers.map { ["name":$0.key,"samples":$0.value.count,"channels":$0.value.channels] }.sorted { ($0["name"] as! String) < ($1["name"] as! String) }])
            case "export":
                let directory = URL(fileURLWithPath:op["directory"] as! String)
                try FileManager.default.createDirectory(at:directory,withIntermediateDirectories:true)
                var assets: [[String:Any]] = []
                for (name,pcm) in audio.buffers.sorted(by:{$0.key < $1.key}) {
                    let path = directory.appendingPathComponent(name+".f32")
                    try Data(bytes:pcm.samples,count:pcm.count*4).write(to:path,options:.atomic)
                    var asset: [String:Any] = ["name":name,"channels":pcm.channels,"samples":pcm.count,"rate":48000,"file":path.path]
                    if let wav = audio.bankWav[name] {
                        let source = directory.appendingPathComponent(name+".wav")
                        try wav.write(to:source,options:.atomic); asset["wav"] = source.path
                    }
                    assets.append(asset)
                }
                events.append(["op":kind,"assets":assets])
            case "asset":
                let bytes = try Data(contentsOf:URL(fileURLWithPath:op["file"] as! String)), channels = (op["channels"] as! NSNumber).uint32Value
                let format = AVAudioFormat(commonFormat:.pcmFormatFloat32,sampleRate:48000,channels:channels,interleaved:true)!
                let buffer = AVAudioPCMBuffer(pcmFormat:format,frameCapacity:AVAudioFrameCount(bytes.count/4/Int(channels)))!
                buffer.frameLength = buffer.frameCapacity
                bytes.withUnsafeBytes { buffer.floatChannelData![0].initialize(from:$0.bindMemory(to:Float.self).baseAddress!,count:bytes.count/4) }
                audio.buffers[op["name"] as! String] = ForgeDecodedPCM(buffer)
            case "reset":
                let assets = audio.buffers, wav = audio.bankWav
                audio.close(); audio = ForgeMetalAudio(host:host,resources:res,muted:true)
                audio.buffers = assets; audio.bankWav = wav; audio.banked = true
            case "drain":
                var count = 0
                let p = mf_sound_commands(host,&count)
                let bytes = p.map { Data(bytes:$0,count:count) } ?? Data()
                var results: [Int64] = []
                for line in bytes.split(separator:10) { results.append(audio.execute(Data(line),contextStarted:op["started"] as? Bool ?? false)) }
                events.append(["op":kind,"text":String(decoding:bytes,as:UTF8.self),"results":results])
            case "probe":
                let result = mf_sound_probe(host,(op["slot"] as! NSNumber).int32Value,(op["right"] as! NSNumber).floatValue,(op["ahead"] as! NSNumber).floatValue,0)
                events.append(["op":kind,"result":result])
            case "command":
                let line = op["line"] as! String
                events.append(["op":kind,"line":line,"result":audio.execute(Data(line.utf8),contextStarted:true)])
            case "render":
                var bytes = Data()
                let blocks = (op["blocks"] as! NSNumber).intValue
                let start = ProcessInfo.processInfo.systemUptime
                for _ in 0..<blocks {
                    guard let block = audio.source.offlineBlock() else { fatalError("audio block refused") }
                    block.withUnsafeBytes { bytes.append(contentsOf:$0) }
                }
                try bytes.write(to:URL(fileURLWithPath:op["file"] as! String),options:.atomic)
                events.append(["op":kind,"blocks":blocks,"bytes":bytes.count,"seconds":ProcessInfo.processInfo.systemUptime-start,"observed":audio.source.observations()])
            case "device":
                audio.start()
                RunLoop.main.run(until:Date().addingTimeInterval(0.2))
                let running = audio.source.observations()
                audio.setPaused(true)
                RunLoop.main.run(until:Date().addingTimeInterval(0.05))
                let paused = audio.source.observations()
                RunLoop.main.run(until:Date().addingTimeInterval(0.1))
                let held = audio.source.observations()
                audio.setPaused(false)
                RunLoop.main.run(until:Date().addingTimeInterval(0.2))
                let resumed = audio.source.observations()
                audio.setPaused(true)
                events.append(["op":kind,"started":audio.started,"muted":audio.engine.mainMixerNode.outputVolume == 0,"running":running,"paused":paused,"held":held,"resumed":resumed])
            default: fatalError("unknown audio effect")
            }
        }
        let report: [String:Any] = ["schema":"aethyr.metal.audio-result.v1","events":events,"observed":audio.source.observations()]
        try JSONSerialization.data(withJSONObject:report,options:[.sortedKeys,.prettyPrinted]).write(to:URL(fileURLWithPath:CommandLine.arguments[4]),options:.atomic)
    }
}
