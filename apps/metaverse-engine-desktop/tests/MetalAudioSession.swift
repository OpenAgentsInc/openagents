// Firm reason: 6 (actual CoreAudio source-buffer and concrete device observations).
import AVFoundation
import Foundation
@main
@MainActor
struct MetalAudioSession {
    static func main() throws {
        guard CommandLine.arguments.count == 2 || CommandLine.arguments.count == 4 else { fatalError("MetalAudioSession REPORT [RESOURCES FIXTURE]") }
        let source=EditorPCMSource()
        let format=AVAudioFormat(standardFormatWithSampleRate:48000,channels:2)!
        let buffer=AVAudioPCMBuffer(pcmFormat:format,frameCapacity:17)!
        buffer.frameLength=17
        for channel in 0..<2 { for i in 0..<17 { buffer.floatChannelData![channel][i]=0.125 } }
        let status=source.render(17,buffer.mutableAudioBufferList)
        let info=source.observations()
        var nonzero=0
        for channel in 0..<2 { for i in 0..<17 { if buffer.floatChannelData![channel][i] != 0 { nonzero += 1 } } }
        source.close()
        var report: [String:Any]=["preFrames":info["frames"] ?? -1,"preDeviceFrames":info["deviceFrames"] ?? -1,"nonzero":nonzero,"status":status]
        if CommandLine.arguments.count == 4 { report["observed"]=try product() }
        try JSONSerialization.data(withJSONObject:report,options:[.sortedKeys]).write(to:URL(fileURLWithPath:CommandLine.arguments[1]),options:.atomic)
    }
    @MainActor final class Realm {
        let host: UnsafeMutableRawPointer
        let audio: EditorMetalAudio
        var closed=false
        init(_ mode: Int32) {
            let resources=URL(fileURLWithPath:CommandLine.arguments[2])
            guard let host=mf_open(resources.path,CommandLine.arguments[3],false,true),mf_audio_fixture_mode(host,mode) else { fatalError("audio recording realm") }
            self.host=host
            audio=EditorMetalAudio(host:host,resources:resources,muted:true,bankDirectory:resources.appendingPathComponent("bank-\(mode)"))
            audio.bank()
            for name in audio.bankWav.keys.sorted() { _ = audio.buffer(name) }
        }
        func send(_ text: String)->Int64 {
            Data(text.utf8).withUnsafeBytes { mf_message(host,930,$0.bindMemory(to:UInt8.self).baseAddress,$0.count) }
            audio.drain();return audio.lastResult
        }
        func frame()->Bool { let count=mf_frame(host,320,180,1,1.0/64);audio.drain();return count }
        func close() { if !closed { audio.close();mf_close(host);closed=true } }
    }
    static func wait(_ seconds: Double) { RunLoop.main.run(until:Date().addingTimeInterval(seconds)) }
    static func callback(_ source: EditorPCMSource, after: Int64, framesAfter: Int64 = -1) throws {
        let deadline=Date().addingTimeInterval(5)
        while Date()<deadline {
            let info=source.observations()
            if info["callbacks",default:0]>after && info["frames",default:0]>framesAfter { return }
            wait(0.01)
        }
        throw NSError(domain:"CoreAudio callback timeout",code:1)
    }
    static func late(_ source: EditorPCMSource)->Int64 {
        let format=AVAudioFormat(standardFormatWithSampleRate:48000,channels:2)!,buffer=AVAudioPCMBuffer(pcmFormat:format,frameCapacity:17)!
        buffer.frameLength=17
        for channel in 0..<2 { for i in 0..<17 { buffer.floatChannelData![channel][i]=0.125 } }
        var bad: Int64=source.render(17,buffer.mutableAudioBufferList)==noErr ? 0:1
        for channel in 0..<2 { for i in 0..<17 { if buffer.floatChannelData![channel][i] != 0 { bad += 1 } } }
        return bad
    }
    static func product() throws -> [Int64] {
        var realm=Realm(0);defer { realm.close() }
        var q=[Int64](repeating:0,count:32)
        q[0]=Int64(realm.audio.buffers.count);q[1]=realm.audio.memory.total
        q[2]=realm.send("!bus 0.5");q[3]=realm.send("pulse 0.25 0");wait(0.1)
        q[4]=realm.audio.source.observations()["frames",default:-1];q[5]=realm.audio.source.observations()["callbacks",default:-1]
        realm.audio.setPaused(true);realm.audio.start();wait(0.1)
        q[6]=realm.audio.source.observations()["frames",default:-1];q[7]=realm.audio.source.observations()["callbacks",default:-1];q[8]=realm.audio.started ? 1:0
        q[23]+=late(realm.audio.source)
        realm.audio.setPaused(false);try callback(realm.audio.source,after:0)
        q[11]=realm.send("pulse 0.25 0");q[12]=realm.audio.source.observations()["accepted",default:-1]
        realm.audio.drain();q[13]=realm.audio.source.observations()["accepted",default:-1]
        realm.audio.setPaused(true);q[9]=realm.audio.source.observations()["frames",default:-1];q[10]=realm.audio.source.observations()["callbacks",default:-1]
        wait(0.15);q[14]=realm.audio.source.observations()["frames",default:-1];q[15]=realm.audio.source.observations()["callbacks",default:-1]
        realm.audio.setPaused(false);try callback(realm.audio.source,after:q[15],framesAfter:q[14]);realm.audio.setPaused(true)
        q[16]=realm.audio.source.observations()["frames",default:-1];q[17]=realm.audio.source.observations()["callbacks",default:-1]
        q[23]+=realm.audio.source.observations()["nonzeroDeviceSamples",default:-1];q[30]=mf_audio_fixture_probe(realm.host,1)
        realm.audio.retire();q[18]=realm.audio.source.sessionOffer().retired ? 1:0;q[19]=realm.audio.source.observations()["frames",default:-1]
        realm.audio.start();realm.audio.setPaused(false);q[23]+=late(realm.audio.source);wait(0.1);q[20]=realm.audio.source.observations()["frames",default:-1]
        q[21]=realm.audio.node == nil && !realm.audio.engine.isRunning ? 1:0;realm.close();q[22]=realm.audio.source.pcm == nil ? 1:0
        realm=Realm(1);q[24]=realm.audio.bankStatus
        guard realm.frame(),realm.frame() else { fatalError("bad metadata froze world") };let metadataFrames=mf_observe(realm.host,-1,4);realm.close()
        realm=Realm(2);q[25]=realm.audio.bankStatus;q[26]=Int64(realm.audio.buffers.count);realm.audio.setPaused(true);realm.audio.start()
        q[27]=realm.send("pulse 0.25 0");q[28]=realm.send("pulse2 0.25 0")
        guard realm.frame(),realm.frame() else { fatalError("missing asset froze world") };q[29]=mf_observe(realm.host,-1,4);guard q[29]==metadataFrames else { fatalError("world frame count differs") };realm.close()
        for mode: Int32 in 3..<6 {
            realm=Realm(mode);let result=realm.send("!bus 0.5")
            if result == -1 && realm.audio.source.observations()["accepted"]==0 && mf_audio_fixture_probe(realm.host,1)==1 { q[31]+=1 }
            guard realm.frame(),realm.frame(),mf_observe(realm.host,-1,4)==2 else { fatalError("hostile packet froze world") };realm.close()
        }
        return q
    }
}
