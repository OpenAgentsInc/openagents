// Firm reason: 6 (actual MF reader/acknowledgment and native receiver callback effects).
import Foundation
import Darwin
@main @MainActor struct MetalActionPacket {
    static func main() throws {
        guard CommandLine.arguments.count == 4 else { fatalError("MetalActionPacket RESOURCES SDK REPORT") }
        var rows: [Int64] = []
        for mode: Int32 in [0,1,2,3,4,5,6,7,8,99] {
            let name = mode == 99 ? "libactionfixture-no-cap.dylib" : "libactionfixture.dylib"
            let path = URL(fileURLWithPath: CommandLine.arguments[2]).appendingPathComponent(name).path
            guard let host = mf_open(CommandLine.arguments[1],path,false,true), let image = dlopen(path,RTLD_NOW|RTLD_LOCAL) else { fatalError(mf_error(nil).map{String(cString:$0)} ?? "action fixture open") }
            defer { dlclose(image); mf_close(host) }
            typealias Mode = @convention(c) (Int32) -> Void
            typealias Probe = @convention(c) (Int32) -> Int64
            guard let rawMode=dlsym(image,"coaxial_action_fixture_mode"),let rawProbe=dlsym(image,"coaxial_action_fixture_probe") else { fatalError("action fixture scalar exports") }
            let set=unsafeBitCast(rawMode,to:Mode.self),probe=unsafeBitCast(rawProbe,to:Probe.self)
            mf_observe_reset(host);set(mode)
            let before=mf_observe(host,-1,4),data=Data("hash".utf8)
            data.withUnsafeBytes{mf_message(host,900,$0.bindMemory(to:UInt8.self).baseAddress,$0.count)}
            let code=mf_action(host);guard code != 0 else { fatalError("missing authored action") }
            var count=0;let pointer=mf_outbox(host,&count),admitted=mf_outbox_admitted(host)
            // This is an actual receiver callback, not clipboard authorization.
            var received: (Int32,String)?
            let text=admitted ? String(decoding:UnsafeBufferPointer(start:pointer,count:count),as:UTF8.self) : ""
            mf_action_done(host)
            if admitted { received=(code,text) }
            let events=(0..<mf_observe(host,-1,2)).map{event in (0..<8).map{mf_observe(host,event,Int64($0))}}
            let acks=events.filter{$0[0]==8},errors=events.filter{$0[0]==4&&$0[1]==604}
            let ack=events.firstIndex{$0[0]==8},error=events.firstIndex{$0[0]==4&&$0[1]==604}
            let ordered=error == nil || (ack != nil && ack! < error!)
            guard mf_frame(host,320,180,1,1.0/64) else { fatalError("producer continuation") }
            let effect=received.map{$0.0==3&&$0.1==(count==0 ? "":"host-contract-v1")} ?? false
            rows += [Int64(mode),0,probe(0),probe(1),probe(2),Int64(acks.count),acks.first?[1] ?? -99,Int64(errors.count),ordered ? 1:0,effect ? 1:0,mf_observe(host,-1,4)>before ? 1:0]
        }
        try JSONSerialization.data(withJSONObject:["schema":"aethyr.host.metal-action-packet.v1","rows":rows],options:[.sortedKeys]).write(to:URL(fileURLWithPath:CommandLine.arguments[3]),options:.atomic)
    }
}
