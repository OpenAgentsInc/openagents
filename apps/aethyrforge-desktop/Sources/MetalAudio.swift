// Firm reason: 6 (AVAudioFile/AVAudioConverter and CoreAudio source-node effects).
import AVFoundation
import Foundation

// Stable decoded memory. Coax borrows it until engine retirement.
final class ForgeDecodedPCM {
    let samples: UnsafeMutablePointer<Float>
    let count: Int
    let channels: Int64
    init(count: Int, channels: Int64) {
        self.count = count; self.channels = channels
        samples = .allocate(capacity:count)
    }
    init(_ buffer: AVAudioPCMBuffer) {
        channels = Int64(buffer.format.channelCount)
        count = Int(buffer.frameLength) * Int(channels)
        samples = .allocate(capacity:count)
        samples.initialize(from:buffer.floatChannelData![0],count:count)
    }
    deinit { samples.deallocate() }
}

// Serialize command delivery with the CoreAudio callback. The callback copies
// whole Coax blocks to device planes without allocating memory.
final class ForgePCMSource: @unchecked Sendable {
    let lock = NSLock()
    var pcm = mf_pcm_open()
    var block: UnsafePointer<Float>?
    var cursor = 1024
    var callbacks = 0
    var rendered = 0
    func command(_ bytes: UnsafeBufferPointer<UInt8>, _ decoded: ForgeDecodedPCM?, started: Bool = true) -> Int64 {
        lock.lock(); defer { lock.unlock() }
        return mf_pcm_command(pcm,bytes.baseAddress,bytes.count,decoded?.samples,decoded?.count ?? 0,decoded?.channels ?? 0,started)
    }
    func render(_ frames: AVAudioFrameCount, _ audio: UnsafeMutablePointer<AudioBufferList>) -> OSStatus {
        lock.lock(); defer { lock.unlock() }
        let buffers = UnsafeMutableAudioBufferListPointer(audio)
        var offset = 0
        while offset < Int(frames) {
            if cursor == 1024 {
                var count = 0
                block = mf_pcm_block(pcm,&count)
                guard count == 2048, block != nil else {
                    for buffer in buffers { if let p = buffer.mData { memset(p,0,Int(buffer.mDataByteSize)) } }
                    return kAudio_ParamError
                }
                cursor = 0
            }
            let count = min(1024-cursor,Int(frames)-offset)
            for channel in 0..<buffers.count {
                guard let destination = buffers[channel].mData?.assumingMemoryBound(to:Float.self) else { continue }
                for i in 0..<count { destination[offset+i] = block![(cursor+i)*2+channel] }
            }
            offset += count; cursor += count
        }
        callbacks += 1; rendered += Int(frames)
        return noErr
    }
    func offlineBlock() -> [Float]? {
        lock.lock(); defer { lock.unlock() }
        var count = 0
        guard let p = mf_pcm_block(pcm,&count), count == 2048 else { return nil }
        return Array(UnsafeBufferPointer(start:p,count:count))
    }
    func observations() -> [String:Int64] {
        lock.lock(); defer { lock.unlock() }
        return ["frames":mf_pcm_info(pcm,0),"accepted":mf_pcm_info(pcm,1),"refused":mf_pcm_info(pcm,2),"roomPartitions":mf_pcm_info(pcm,3),"voices":mf_pcm_info(pcm,4),"callbacks":Int64(callbacks),"deviceFrames":Int64(rendered)]
    }
    func close() {
        lock.lock(); defer { lock.unlock() }
        if let pcm { mf_pcm_close(pcm) }; pcm = nil; block = nil
    }
    deinit { if let pcm { mf_pcm_close(pcm) } }
}

@MainActor
final class ForgeMetalAudio {
    let host: UnsafeMutableRawPointer
    let resources: URL
    let engine = AVAudioEngine()
    let source = ForgePCMSource()
    let muted: Bool
    let wav = mf_wav_open()
    var node: AVAudioSourceNode?
    var started = false
    var banked = false
    var buffers: [String:ForgeDecodedPCM] = [:]
    var bankWav: [String:Data] = [:]
    var packets = 0
    var scheduled = 0
    init(host: UnsafeMutableRawPointer, resources: URL, muted: Bool) {
        self.host = host; self.resources = resources; self.muted = muted
        let source = self.source
        let format = AVAudioFormat(standardFormatWithSampleRate:48000,channels:2)!
        let node = AVAudioSourceNode(format:format) { _, _, frames, audio in source.render(frames,audio) }
        self.node = node
        engine.attach(node); engine.connect(node,to:engine.mainMixerNode,format:format)
        engine.mainMixerNode.outputVolume = muted ? 0:1
    }
    func start() {
        guard !started else { return }
        bank()
        do { try engine.start(); started = true } catch { return }
    }
    func bank() {
        guard !banked else { return }; banked = true
        var n = 0
        guard let p = mf_sound_bank(host,&n) else { return }
        let text = String(decoding:UnsafeBufferPointer(start:p,count:n),as:UTF8.self)
        let root = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/aethyrforge/audio-bank")
        try? FileManager.default.createDirectory(at:root,withIntermediateDirectories:true)
        for line in text.split(separator:"\n") {
            let columns = line.split(separator:" ")
            guard columns.count == 3, let address = UInt(columns[1]), let count = Int(columns[2]), let bytes = UnsafeRawPointer(bitPattern:address), count > 0 else { continue }
            let name = String(columns[0]), path = root.appendingPathComponent("\(name).wav")
            let data = Data(bytes:bytes,count:count)
            try? data.write(to:path,options:.atomic)
            bankWav[name] = data
            buffers[name] = decodeCanonical(data)
        }
    }
    func decodeCanonical(_ bytes: Data) -> ForgeDecodedPCM? {
        bytes.withUnsafeBytes { raw in
            let input = raw.bindMemory(to:UInt8.self)
            let count = mf_wav_info(input.baseAddress,input.count,0)
            let channels = mf_wav_info(input.baseAddress,input.count,1)
            guard count > 0, channels > 0 else { return nil }
            let pcm = ForgeDecodedPCM(count:Int(count),channels:channels)
            guard mf_wav_decode(wav,input.baseAddress,input.count,pcm.samples,pcm.count) else { return nil }
            return pcm
        }
    }
    func decode(_ url: URL) -> ForgeDecodedPCM? {
        guard let file = try? AVAudioFile(forReading:url), file.length > 0, file.length <= Int64(UInt32.max),
              let input = AVAudioPCMBuffer(pcmFormat:file.processingFormat,frameCapacity:AVAudioFrameCount(file.length)),
              let format = AVAudioFormat(commonFormat:.pcmFormatFloat32,sampleRate:48000,channels:min(2,file.processingFormat.channelCount),interleaved:true),
              let converter = AVAudioConverter(from:file.processingFormat,to:format) else { return nil }
        do { try file.read(into:input) } catch { return nil }
        let capacity = ceil(Double(input.frameLength)*48000/file.processingFormat.sampleRate)+1024
        guard capacity <= Double(UInt32.max), let output = AVAudioPCMBuffer(pcmFormat:format,frameCapacity:AVAudioFrameCount(capacity)) else { return nil }
        var supplied = false, error: NSError?
        let status = converter.convert(to:output,error:&error) { _, state in
            if supplied { state.pointee = .endOfStream; return nil }
            supplied = true; state.pointee = .haveData; return input
        }
        guard status != .error, error == nil, output.frameLength > 0 else { return nil }
        return ForgeDecodedPCM(output)
    }
    func buffer(_ name: String) -> ForgeDecodedPCM? {
        if let pcm = buffers[name] { return pcm }
        let bytes = Data(name.utf8)
        guard bytes.withUnsafeBytes({ mf_audio_asset_name($0.bindMemory(to:UInt8.self).baseAddress,$0.count) }) else { return nil }
        let pcm = decode(resources.appendingPathComponent("web/realms/everglade/sfx/\(name).ogg"))
        buffers[name] = pcm; return pcm
    }
    @discardableResult
    func execute(_ line: Data, contextStarted: Bool? = nil) -> Int64 {
        line.withUnsafeBytes { raw in
            let bytes = raw.bindMemory(to:UInt8.self)
            var effect = MFAudioEffect()
            guard mf_audio_decode(bytes.baseAddress,bytes.count,&effect) else { return -1 }
            let name = effect.name.map { String(decoding:UnsafeBufferPointer(start:$0,count:effect.name_count),as:UTF8.self) } ?? ""
            let pcm = name.isEmpty ? nil:buffer(name)
            let result = source.command(bytes,pcm,started:contextStarted ?? started)
            packets += 1; if result >= 0 && effect.kind <= 2 { scheduled += 1 }
            return result
        }
    }
    func drain() {
        // Coax retains startup room/bus/bed controls and refuses stale cues
        // while CoreAudio waits for the first real input gesture.
        bank()
        var n = 0
        guard let p = mf_sound_commands(host,&n), n > 0 else { return }
        for line in Data(bytes:p,count:n).split(separator:10) { execute(Data(line)) }
    }
    func setPaused(_ value: Bool) { if value { engine.pause() } else if started { try? engine.start() } }
    func close() {
        engine.stop(); source.close()
        if let node { engine.detach(node) }; node = nil
        buffers.removeAll(); started = false
        bankWav.removeAll()
    }
    deinit { mf_wav_close(wav) }
}
