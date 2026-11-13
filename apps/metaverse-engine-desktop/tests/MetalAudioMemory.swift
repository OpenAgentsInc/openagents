// Firm reason: 6 (actual native allocation, pointer-span and retained-lifetime observations).
import AVFoundation
import Foundation

@main
@MainActor
struct MetalAudioMemory {
    static func main() throws {
        guard CommandLine.arguments.count == 2 || CommandLine.arguments.count == 5 else { fatalError("MetalAudioMemory REPORT [RESOURCES PRODUCER GOLDEN_PCM]") }
        let memory = EditorAudioMemory()
        var rows: [Int64] = []
        for (frames,channels): (Int64,Int64) in [(-1,1),(0,1),(Int64.max,1),(1,0),(1,3),(4_194_305,2)] {
            rows.append(EditorDecodedPCM(frames:frames,channels:channels,memory:memory) == nil ? 1:0)
        }
        rows.append(memory.total)
        var full = EditorDecodedPCM(frames:4_194_304,channels:2,memory:memory)
        rows.append(Int64(full?.count ?? -1)); rows.append(memory.total)
        rows.append(EditorDecodedPCM(frames:1,channels:1,memory:memory) == nil ? 1:0)
        full = nil; rows.append(memory.total)
        var tiny: [EditorDecodedPCM] = []
        for _ in 0..<128 { if let pcm = EditorDecodedPCM(frames:1,channels:1,memory:memory) { tiny.append(pcm) } }
        rows.append(Int64(tiny.count)); rows.append(memory.total)
        rows.append(EditorDecodedPCM(frames:1,channels:1,memory:memory) == nil ? 1:0)
        tiny.removeLast()
        if let pcm = EditorDecodedPCM(frames:1,channels:2,memory:memory) { tiny.append(pcm) }
        rows.append(memory.total)
        tiny.removeAll(); rows.append(memory.total)
        let format = AVAudioFormat(commonFormat:.pcmFormatFloat32,sampleRate:48000,channels:2,interleaved:true)!
        let sdk = AVAudioPCMBuffer(pcmFormat:format,frameCapacity:3)!
        sdk.frameLength = 3
        for i in 0..<6 { sdk.floatChannelData![0][i] = Float(i)/8 }
        var copy = EditorDecodedPCM(sdk,memory:memory)
        rows.append(Int64(copy?.count ?? -1)); rows.append(memory.total)
        rows.append(copy?.samples[5] == 0.625 ? 1:0)
        copy = nil; rows.append(memory.total)
        var bankRows: [[Int64]] = []
        let storage = (37..<81).map { UInt8($0) }
        storage.withUnsafeBufferPointer { source in
            let bank = mf_audio_bank_open(source.baseAddress,source.count)!
            defer { mf_audio_bank_close(bank) }
            let base = UInt(bitPattern:source.baseAddress!)
            let lines = ["x \(base) 44","x \(base) 44","y \(base-1) 1","y \(base+44) 1","y \(base) 45","y \(base) -1","y \(base) 0","../y \(base) 1","y \(base) 1 extra","y \(base+43) 1"]
            for line in lines {
                var asset = MFAudioBankAsset()
                let result = Data(line.utf8).withUnsafeBytes { mf_audio_bank_register(bank,$0.bindMemory(to:UInt8.self).baseAddress,$0.count,&asset) }
                // Only read the admitted pointers; refused outputs remain empty.
                bankRows.append([result ? 1:0,mf_audio_bank_count(bank),Int64(asset.byte_count),asset.name.map { Int64($0.pointee) } ?? 0,asset.bytes.map { Int64($0.pointee) } ?? 0])
            }
        }
        var report: [String:Any] = ["schema":"aethyr.metal.audio-memory.v1","memory":rows,"bank":bankRows]
        if CommandLine.arguments.count == 5 {
            let resources = URL(fileURLWithPath:CommandLine.arguments[2]), golden = URL(fileURLWithPath:CommandLine.arguments[4])
            guard let host = mf_open(resources.path,CommandLine.arguments[3],true,true) else { fatalError("bank producer") }
            defer { mf_close(host) }
            var metadata = 0, capacity = 0
            _ = mf_sound_bank(host,&metadata); _ = mf_sound_bank_storage(host,&capacity)
            let audio = EditorMetalAudio(host:host,resources:resources,muted:true,memory:memory)
            defer { audio.close() }
            audio.bank()
            for name in audio.bankWav.keys.sorted() { _ = audio.buffer(name) }
            let output = URL(fileURLWithPath:CommandLine.arguments[1]).deletingLastPathComponent().appendingPathComponent("decoded")
            try FileManager.default.createDirectory(at:output,withIntermediateDirectories:true)
            var files: [[String:Any]] = [], words: Int64 = 0
            for (name,pcm) in audio.buffers.sorted(by:{$0.key < $1.key}) {
                let samples = Data(bytes:pcm.samples,count:pcm.count*4), wav = audio.bankWav[name]!
                let actual = output.appendingPathComponent(name+".f32"), source = output.appendingPathComponent(name+".wav")
                try samples.write(to:actual,options:.atomic); try wav.write(to:source,options:.atomic)
                files.append(["name":name,"samples":pcm.count,"channels":pcm.channels,"actual":actual.path,"golden":golden.appendingPathComponent(name+".f32").path,"wav":source.path,"goldenWav":golden.appendingPathComponent(name+".wav").path])
                words += Int64(pcm.count)
            }
            var observed = [Int64(metadata),Int64(capacity),Int64(audio.bankWav.count),Int64(audio.buffers.count),memory.total,words]
            var retained = audio.buffers
            withExtendedLifetime(retained) { audio.close(); observed.append(memory.total) }
            retained.removeAll(); observed.append(memory.total)
            var full = EditorDecodedPCM(frames:4_194_304,channels:2,memory:memory)
            let refused = EditorMetalAudio(host:host,resources:resources,muted:true,memory:memory)
            withExtendedLifetime(full) {
                refused.bank(); observed.append(Int64(refused.bankWav.count)); observed.append(Int64(refused.buffers.count)); observed.append(refused.buffer("fire-hit") == nil ? 1:0)
                refused.close()
            }
            full = nil; observed.append(memory.total)
            var converted = audio.decode(golden.appendingPathComponent("fire-hit.wav"))
            withExtendedLifetime(converted) { observed.append(Int64(converted?.count ?? -1)); observed.append(memory.total) }
            converted = nil; observed.append(memory.total)
            report["canonical"] = observed; report["files"] = files
        }
        try JSONSerialization.data(withJSONObject:report,options:[.sortedKeys,.prettyPrinted]).write(to:URL(fileURLWithPath:CommandLine.arguments[1]),options:.atomic)
    }
}
