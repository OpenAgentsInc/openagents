// Firm reason: 6 (observe the actual matching realm face's additive storage export).
import Foundation
import Darwin
@main @MainActor struct MetalActionCapacity {
    static func main() throws {
        guard CommandLine.arguments.count==4,let host=mf_open(CommandLine.arguments[1],CommandLine.arguments[2],false,true),let image=dlopen(CommandLine.arguments[2],RTLD_NOW|RTLD_LOCAL) else { fatalError("matching realm face open") }
        defer{dlclose(image);mf_close(host)}
        typealias Capacity = @convention(c) (UInt)->UInt32
        typealias Address = @convention(c) (UInt)->UInt
        typealias Version = @convention(c) ()->UInt32
        guard let cap=dlsym(image,"coaxial_outbox_capacity"),let address=dlsym(image,"coaxial_outbox"),let version=dlsym(image,"coaxial_host_contract_version") else { fatalError("capacity export absent") }
        var stateBytes=0;guard let state=mf_cpu_bytes(host,8,&stateBytes) else { fatalError("measured state allocation") }
        let base=UInt(bitPattern:state),count=UInt(unsafeBitCast(cap,to:Capacity.self)(base)),at=unsafeBitCast(address,to:Address.self)(base),v=unsafeBitCast(version,to:Version.self)()
        guard v==1,count>0,at>=base,at-base<=UInt(stateBytes),count<=UInt(stateBytes)-(at-base) else { fatalError("capacity is outside actual producer storage") }
        var bytes=0;_ = mf_outbox(host,&bytes)
        guard bytes==0,mf_outbox_admitted(host) else { fatalError("initial empty action") }
        try JSONSerialization.data(withJSONObject:["schema":"aethyr.host.metal-action-capacity.v1","version":v,"stateBytes":stateBytes,"outboxCapacity":count,"withinMeasuredState":true,"emptyAdmitted":true],options:[.sortedKeys]).write(to:URL(fileURLWithPath:CommandLine.arguments[3]),options:.atomic)
    }
}
