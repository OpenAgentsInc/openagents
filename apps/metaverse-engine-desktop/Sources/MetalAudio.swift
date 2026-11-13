// Firm reason: 6 (AVAudioFile/AVAudioConverter and CoreAudio source-node effects).
import AVFoundation
import Foundation

// Tickets live as long as the allocations they admit, including SDK converter
// scratch and samples borrowed by a retiring audio context.
final class EditorAudioMemory {
    private let lock = NSLock()
    private let state = mf_audio_memory_open()
    func reserve(frames: Int64, channels: Int64) -> EditorAudioReservation? {
        let words = mf_audio_extent(frames,channels)
        lock.lock(); defer { lock.unlock() }
        let ticket = mf_audio_memory_reserve(state,words)
        guard ticket >= 0 else { return nil }
        return EditorAudioReservation(memory:self,ticket:ticket,words:mf_audio_memory_words(state,ticket))
    }
    fileprivate func release(_ ticket: Int64) {
        lock.lock(); defer { lock.unlock() }
        _ = mf_audio_memory_release(state,ticket)
    }
    var total: Int64 {
        lock.lock(); defer { lock.unlock() }
        return mf_audio_memory_total(state)
    }
    deinit { mf_audio_memory_close(state) }
}
final class EditorAudioReservation {
    let memory: EditorAudioMemory
    let ticket: Int64
    let words: Int64
    fileprivate init(memory: EditorAudioMemory, ticket: Int64, words: Int64) {
        self.memory = memory; self.ticket = ticket; self.words = words
    }
    deinit { memory.release(ticket) }
}

// The reservation precedes the SDK read and survives every borrowed byte view.
final class EditorEncodedAudio {
    let data: Data
    private let reservation: EditorAudioReservation
    init?(_ url: URL, companion: Bool, memory: EditorAudioMemory) {
        guard let file = try? FileHandle(forReadingFrom:url) else { return nil }
        defer { try? file.close() }
        guard let extent = try? file.seekToEnd(), let count = Int64(exactly:extent) else { return nil }
        let words = mf_audio_encoded_words(count,companion)
        guard let ticket = memory.reserve(frames:words,channels:1) else { return nil }
        do {
            try file.seek(toOffset:0)
            guard let bytes = try file.read(upToCount:Int(count)), bytes.count == count,
                  try file.seekToEnd() == extent else { return nil }
            data = bytes; reservation = ticket
        } catch { return nil }
    }
}

// Stable decoded memory. Coax borrows it until engine retirement.
final class EditorDecodedPCM {
    let samples: UnsafeMutablePointer<Float>
    let count: Int
    let channels: Int64
    private let reservation: EditorAudioReservation
    init?(frames: Int64, channels: Int64, memory: EditorAudioMemory) {
        guard let receipt = memory.reserve(frames:frames,channels:channels),
              let bytes = calloc(Int(receipt.words),MemoryLayout<Float>.stride) else { return nil }
        reservation = receipt; count = Int(receipt.words); self.channels = channels
        samples = bytes.assumingMemoryBound(to:Float.self)
    }
    convenience init?(_ buffer: AVAudioPCMBuffer, memory: EditorAudioMemory) {
        guard buffer.format.isInterleaved, let source = buffer.floatChannelData?[0] else { return nil }
        self.init(frames:Int64(buffer.frameLength),channels:Int64(buffer.format.channelCount),memory:memory)
        samples.update(from:source,count:count)
    }
    deinit { free(samples) }
}

// Serialize command delivery with the CoreAudio callback. The callback copies
// whole Coax blocks to device planes without allocating memory.
final class EditorPCMSource: @unchecked Sendable {
    let lock = NSLock()
    private let session = mf_audio_session_open()
    var pcm = mf_pcm_open()
    var muted = false
    var block: UnsafePointer<Float>?
    var cursor = 1024
    var callbacks = 0
    var rendered = 0
    var attempts = 0
    var nonzeroDeviceSamples = 0
    private var retiredInfo: [String:Int64] = [:]
    func sessionOffer() -> MFAudioSessionOffer {
        lock.lock(); defer { lock.unlock() }; return mf_audio_session_offer(session)
    }
    @discardableResult
    func sessionEvent(_ kind: Int64) -> MFAudioSessionOffer {
        lock.lock(); defer { lock.unlock() }; return mf_audio_session_event(session,kind)
    }
    func command(_ bytes: UnsafeBufferPointer<UInt8>, _ decoded: EditorDecodedPCM?, started: Bool? = nil) -> Int64 {
        lock.lock(); defer { lock.unlock() }
        return mf_pcm_command(pcm,bytes.baseAddress,bytes.count,decoded?.samples,decoded?.count ?? 0,decoded?.channels ?? 0,started ?? mf_audio_session_offer(session).started)
    }
    func render(_ frames: AVAudioFrameCount, _ audio: UnsafeMutablePointer<AudioBufferList>) -> OSStatus {
        lock.lock(); defer { lock.unlock() }
        let buffers = UnsafeMutableAudioBufferListPointer(audio)
        attempts += 1
        guard mf_audio_session_offer(session).active else {
            for buffer in buffers { if let p = buffer.mData { memset(p,0,Int(buffer.mDataByteSize)) } }
            return noErr
        }
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
                for i in 0..<count { destination[offset+i] = muted ? 0:block![(cursor+i)*2+channel] }
            }
            offset += count; cursor += count
        }
        for buffer in buffers { if let bytes=buffer.mData?.assumingMemoryBound(to:Float.self) { for i in 0..<(Int(buffer.mDataByteSize)/MemoryLayout<Float>.stride) { if bytes[i] != 0 { nonzeroDeviceSamples += 1 } } } }
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
        var info = pcm == nil ? retiredInfo:pcmInfo()
        let offer = mf_audio_session_offer(session)
        info["callbacks"]=Int64(callbacks);info["deviceFrames"]=Int64(rendered);info["attemptedCallbacks"]=Int64(attempts)
        info["active"]=offer.active ? 1:0;info["started"]=offer.started ? 1:0;info["retired"]=offer.retired ? 1:0
        info["nonzeroDeviceSamples"]=Int64(nonzeroDeviceSamples)
        return info
    }
    private func pcmInfo() -> [String:Int64] { ["frames":mf_pcm_info(pcm,0),"accepted":mf_pcm_info(pcm,1),"refused":mf_pcm_info(pcm,2),"roomPartitions":mf_pcm_info(pcm,3),"voices":mf_pcm_info(pcm,4)] }
    func close() {
        lock.lock(); defer { lock.unlock() }
        _ = mf_audio_session_event(session,4)
        if let pcm { retiredInfo=pcmInfo(); mf_pcm_close(pcm) }; pcm = nil; block = nil
    }
    deinit { if let pcm { mf_pcm_close(pcm) };mf_audio_session_close(session) }
}

@MainActor
final class EditorMetalAudio {
    let host: UnsafeMutableRawPointer
    let resources: URL
    let engine = AVAudioEngine()
    let source = EditorPCMSource()
    let muted: Bool
    let memory: EditorAudioMemory
    let bankDirectory: URL
    let wav = mf_wav_open()
    var node: AVAudioSourceNode?
    var started: Bool { source.sessionOffer().started }
    var banked = false
    var buffers: [String:EditorDecodedPCM] = [:]
    var bankWav: [String:Data] = [:]
    var packets = 0
    var scheduled = 0
    var lastResult: Int64 = -1
    var bankStatus: Int64 = -1
    var sdkDecodes = 0
    var companionDecodes = 0
    init(host: UnsafeMutableRawPointer, resources: URL, muted: Bool, memory: EditorAudioMemory = EditorAudioMemory(), bankDirectory: URL? = nil) {
        self.host = host; self.resources = resources; self.muted = muted; self.memory = memory
        self.bankDirectory = bankDirectory ?? URL(fileURLWithPath: NSHomeDirectory()).appendingPathComponent(".openagents/aethyrforge/audio-bank")
        let source = self.source
        source.muted = muted
        let format = AVAudioFormat(standardFormatWithSampleRate:48000,channels:2)!
        let node = AVAudioSourceNode(format:format) { _, _, frames, audio in source.render(frames,audio) }
        self.node = node
        engine.attach(node); engine.connect(node,to:engine.mainMixerNode,format:format)
        engine.mainMixerNode.outputVolume = muted ? 0:1
    }
    func start() {
        guard !source.sessionOffer().retired else { return }
        bank()
        if source.sessionEvent(1).active && !engine.isRunning { try? engine.start() }
    }
    func bank() {
        guard !banked else { return }; banked = true
        var n = 0, capacity = 0
        guard let p = mf_sound_bank(host,&n), let base = mf_sound_bank_storage(host,&capacity),
              let registry = mf_audio_bank_open(base,capacity) else { return }
        bankStatus = 0
        defer { mf_audio_bank_close(registry) }
        let root = bankDirectory
        try? FileManager.default.createDirectory(at:root,withIntermediateDirectories:true)
        for line in Data(bytes:p,count:n).split(separator:10) {
            var asset = MFAudioBankAsset()
            let accepted = line.withUnsafeBytes { mf_audio_bank_register(registry,$0.bindMemory(to:UInt8.self).baseAddress,$0.count,&asset) }
            guard accepted, let nameBytes = asset.name, let bytes = asset.bytes else { bankStatus = -1; continue }
            let name = String(decoding:UnsafeBufferPointer(start:nameBytes,count:asset.name_count),as:UTF8.self)
            let path = root.appendingPathComponent("\(name).wav")
            let data = Data(bytes:bytes,count:asset.byte_count)
            try? data.write(to:path,options:.atomic)
            bankWav[name] = data
            // Register canonical bytes now; only a Coax effect requesting this
            // name needs a decoded PCM allocation. Default effects request the
            // authored Ogg variants rather than the unused synthesized bank.
            if data.withUnsafeBytes({ mf_wav_info($0.bindMemory(to:UInt8.self).baseAddress,$0.count,0) }) < 1 { bankStatus = -1 }
        }
    }
    func decodeCanonical(_ bytes: Data) -> EditorDecodedPCM? {
        bytes.withUnsafeBytes { raw in
            let input = raw.bindMemory(to:UInt8.self)
            let frames = mf_wav_info(input.baseAddress,input.count,2)
            let channels = mf_wav_info(input.baseAddress,input.count,1)
            guard let pcm = EditorDecodedPCM(frames:frames,channels:channels,memory:memory) else { return nil }
            guard mf_wav_decode(wav,input.baseAddress,input.count,pcm.samples,pcm.count) else { return nil }
            return pcm
        }
    }
    func decode(_ url: URL, admitted: MFOpusPlan? = nil) -> EditorDecodedPCM? {
        guard let file = try? AVAudioFile(forReading:url),
              admitted.map({ mf_audio_opus_output($0,file.length,Int64(file.processingFormat.channelCount),file.processingFormat.sampleRate) }) ?? true,
              let inputTicket = memory.reserve(frames:admitted?.decode_frames ?? file.length,channels:Int64(file.processingFormat.channelCount)),
              let inputCapacity = AVAudioFrameCount(exactly:admitted?.decode_frames ?? file.length),
              let input = AVAudioPCMBuffer(pcmFormat:file.processingFormat,frameCapacity:inputCapacity),
              let format = AVAudioFormat(commonFormat:.pcmFormatFloat32,sampleRate:48000,channels:file.processingFormat.channelCount,interleaved:true),
              let converter = AVAudioConverter(from:file.processingFormat,to:format) else { return nil }
        do { try file.read(into:input) } catch { return nil }
        guard admitted.map({ mf_audio_opus_output($0,Int64(input.frameLength),Int64(input.format.channelCount),input.format.sampleRate) }) ?? true else { return nil }
        let capacity = ceil(Double(input.frameLength)*48000/file.processingFormat.sampleRate)+1024
        guard let outputFrames = Int64(exactly:capacity),
              let outputTicket = memory.reserve(frames:outputFrames,channels:Int64(format.channelCount)),
              let output = AVAudioPCMBuffer(pcmFormat:format,frameCapacity:AVAudioFrameCount(capacity)) else { return nil }
        defer { withExtendedLifetime(inputTicket) {}; withExtendedLifetime(outputTicket) {} }
        var supplied = false, error: NSError?
        let status = converter.convert(to:output,error:&error) { _, state in
            if supplied { state.pointee = .endOfStream; return nil }
            supplied = true; state.pointee = .haveData; return input
        }
        guard status != .error, error == nil, output.frameLength > 0,
              admitted.map({ mf_audio_opus_output($0,Int64(output.frameLength),Int64(output.format.channelCount),output.format.sampleRate) }) ?? true else { return nil }
        sdkDecodes += 1
        return EditorDecodedPCM(output,memory:memory)
    }
    func loadAsset(_ name: String) -> EditorDecodedPCM? {
        let nameBytes = Data(name.utf8)
        guard nameBytes.withUnsafeBytes({ mf_audio_asset_name($0.bindMemory(to:UInt8.self).baseAddress,$0.count) }) else { return nil }
        var asset = MFAudioAsset()
        let descriptor = nameBytes.withUnsafeBytes { mf_audio_asset_open($0.bindMemory(to:UInt8.self).baseAddress,$0.count,&asset) }
        defer { mf_audio_asset_close(descriptor) }
        guard !asset.valid || descriptor != nil else { return nil }
        let sourcePath = asset.source_url.map { String(decoding:UnsafeBufferPointer(start:$0,count:asset.source_url_count),as:UTF8.self) } ?? "/realms/everglade/sfx/\(name).ogg"
        let url = resources.appendingPathComponent("web"+sourcePath)
        guard let source = EditorEncodedAudio(url,companion:false,memory:memory) else { return nil }
        defer { withExtendedLifetime(source) {} }
        let plan = source.data.withUnsafeBytes { mf_audio_opus_plan(descriptor,$0.bindMemory(to:UInt8.self).baseAddress,$0.count) }
        guard plan.valid else { return nil }
        if asset.canonical {
            guard let pathBytes = asset.companion_url else { return nil }
            let path = String(decoding:UnsafeBufferPointer(start:pathBytes,count:asset.companion_url_count),as:UTF8.self)
            guard let companion = EditorEncodedAudio(resources.appendingPathComponent("web"+path),companion:true,memory:memory) else { return nil }
            defer { withExtendedLifetime(companion) {} }
            return source.data.withUnsafeBytes { rawSource in
                companion.data.withUnsafeBytes { rawCompanion in
                    let input = rawSource.bindMemory(to:UInt8.self), encoded = rawCompanion.bindMemory(to:UInt8.self)
                    let shape = mf_audio_companion_plan(descriptor,input.baseAddress,input.count,encoded.baseAddress,encoded.count)
                    guard shape.valid, let pcm = EditorDecodedPCM(frames:shape.frames,channels:shape.channels,memory:memory),
                          mf_audio_companion_decode(descriptor,input.baseAddress,input.count,encoded.baseAddress,encoded.count,pcm.samples,pcm.count) else { return nil }
                    companionDecodes += 1; return pcm
                }
            }
        }
        return decode(url,admitted:plan)
    }
    func buffer(_ name: String) -> EditorDecodedPCM? {
        if let pcm = buffers[name] { return pcm }
        if let bytes = bankWav[name] {
            let pcm = decodeCanonical(bytes)
            buffers[name] = pcm
            if pcm == nil { bankStatus = -1 }
            // A refused canonical allocation never acquires an SDK substitute.
            return pcm
        }
        let pcm = loadAsset(name)
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
            let result = source.command(bytes,pcm,started:contextStarted)
            lastResult = result
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
    func setPaused(_ value: Bool) {
        if source.sessionEvent(value ? 2:3).active { if !engine.isRunning { try? engine.start() } } else { engine.pause() }
    }
    func retire() {
        source.sessionEvent(4);engine.stop()
        if let node { engine.detach(node) }; node = nil
    }
    func close() {
        retire(); source.close()
        buffers.removeAll()
        bankWav.removeAll()
    }
    deinit { mf_wav_close(wav) }
}
