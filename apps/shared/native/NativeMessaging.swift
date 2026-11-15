// Firm reason: 6 (authenticated HTTP, Keychain custody, entropy and owned worker SDK effects).
// Protocol, verification, encryption, intent digests and admission are Coax.
import UIKit
import Security

private enum NativeMessagingWorker {
    // Crypto's existing bounded workspaces need more than a cooperative
    // executor thread's stack. The owner retains every input until completion.
    static func run<T>(_ work:@escaping()throws->T)async throws->T {
        try await withCheckedThrowingContinuation {continuation in
            let thread=Thread {do {continuation.resume(returning:try work())}catch {continuation.resume(throwing:error)}}
            thread.name="OpenAgents Nostr custody";thread.stackSize=8*1024*1024;thread.start()
        }
    }
}

@MainActor final class NativeMessagingServices {
    let auth:AppAuth
    private let account:NativeAccountServices
    private let transport=NativeWorldSession()
    var send:AppAuth.Send
    private var epoch=UUID()
    private var requests=[UUID:Task<(Data,Int),Error>]()
    // Installed relay controller supplies actual current challenge/read facts.
    // Absence refuses AUTH/list writes; the updateable caller cannot set it.
    var admitCustody:((String)async throws->String)?
    init(auth:AppAuth,account:NativeAccountServices,send:AppAuth.Send?=nil) {
        self.auth=auth;self.account=account
        let transport=self.transport;self.send=send ?? {try await transport.fetch($0,limit:400000)}
    }
    func retire() {epoch=UUID();for request in requests.values {request.cancel()};requests.removeAll()}
    private struct Owner {let epoch:UUID;let login:UUID;let account:String}
    private func owner()throws->Owner {guard let current=auth.tokens?.account,!current.isEmpty else {throw AppAuth.Failure.said("Log in first.")};return Owner(epoch:epoch,login:auth.sessionOwner,account:current)}
    private func current(_ owner:Owner)throws {guard owner.epoch==epoch,owner.login==auth.sessionOwner,owner.account==auth.tokens?.account else {throw CancellationError()}}
    private func request(_ path:String,_ body:String?,_ owner:Owner)async throws->String {
        try current(owner);guard let url=URL(string:auth.origin+path) else {throw AppAuth.Failure.unreachable}
        var q=URLRequest(url:url,cachePolicy:.reloadIgnoringLocalAndRemoteCacheData,timeoutInterval:30)
        if let body {q.httpMethod="POST";q.httpBody=Data(body.utf8);q.setValue("application/json",forHTTPHeaderField:"Content-Type");q.setValue(auth.origin,forHTTPHeaderField:"Origin")}
        q=try await auth.authorize(q);try current(owner)
        let authorized=q,sender=send,id=UUID(),task=Task {try await sender(authorized)}
        requests[id]=task;defer {requests[id]=nil}
        let (data,status)=try await task.value;try current(owner)
        guard data.count<=400000,let raw=String(data:data,encoding:.utf8) else {throw NativeFailure.refused("Invalid Nostr response")}
        guard (200..<300).contains(status) else {throw NativeAccountHTTP.refused(status,AppAuth.errorWords(data) ?? "Nostr unavailable")};return raw
    }
    private func source(_ identity:String,_ owner:Owner)async throws->Int64 {
        let raw=try await request("/api/auth/identity/v1",nil,owner)
        let source=NativeAccountCodec.text(raw) {r in NativeAccountCodec.text(owner.account) {a in NativeAccountCodec.text(identity) {nsm_source(r,a,$0)}}}
        guard source>0 else {retire();throw NativeFailure.refused("Identity changed")};return source
    }
    private func response(_ raw:String,service:Int64,action:Int64,operation:String,identity:String)throws->String {
        let p=UnsafeMutablePointer<coax_t_NsResult>.allocate(capacity:1);p.initialize(to:coax_t_NsResult());defer {p.deinitialize(count:1);p.deallocate()}
        guard NativeAccountCodec.text(raw,{nsr_read($0,coax_v_t_NsResult(data:p,count:1))}),NativeAccountCodec.text(operation,{o in NativeAccountCodec.text(identity) {nsm_reply(coax_v_t_NsResult(data:p,count:1),service,action,o,$0)}}) else {throw NativeFailure.refused("Invalid Nostr response")};return raw
    }
    private func part<T>(_ value:T,_ count:Int)->String {withUnsafeBytes(of:value) {String(decoding:$0.prefix(count),as:UTF8.self)}}
    private func storeRequest(_ action:String,_ identity:String,relay:String="",revision:Int64=0,cursor:Int64=0,payload:String="")throws->String {
        let op=try NativeAccountCodec.random(32).map {String(format:"%02x",$0)}.joined()
        return try NativeAccountCodec.write(400000) {out in NativeAccountCodec.text(action) {a in NativeAccountCodec.text(op) {o in NativeAccountCodec.text(identity) {i in NativeAccountCodec.text(relay) {r in NativeAccountCodec.text(payload) {nms_request_write(a,o,i,r,coax_v_string(data:nil,count:0),0,revision,cursor,$0,out)}}}}}}
    }
    private func stored(_ action:String,_ identity:String,relay:String="",revision:Int64=0,cursor:Int64=0,payload:String="")async throws->String {
        let value=try await store(try storeRequest(action,identity,relay:relay,revision:revision,cursor:cursor,payload:payload))
        guard let raw=value["record"] as? String,NativeAccountCodec.part(raw,"status")=="ok" else {throw NativeFailure.refused("Nostr store not ready")};return raw
    }
    func relaySettings(_ identity:String)async throws->String {
        let owner=try self.owner();_ = try await source(identity,owner);_ = try await stored("snapshot",identity)
        return try await request("/api/auth/settings/v1",nil,owner)
    }
    // Receive ordering is preserved by the socket reader awaiting this commit.
    func received(_ frame:String,identity:String,relay:String)async throws {
        let kind=NativeAccountCodec.text(frame) {nsm_frame_type($0)}
        if kind==1 {
            let original=try NativeAccountCodec.write(65536) {out in NativeAccountCodec.text(frame) {nsm_incoming($0,out)}}
            _ = try await stored("ingest",identity,relay:relay,payload:original)
        }else if kind==2 {_ = try await stored("ack",identity,relay:relay,payload:frame)}
    }
    func staged(_ original:String,identity:String,relay:String)async throws->String {
        let id=NativeAccountCodec.part(original,"id");var revision:Int64=0,cursor:Int64=0
        for _ in 0..<4 {
            let raw=try await stored("outbox",identity,revision:revision,cursor:cursor)
            let r=UnsafeMutablePointer<coax_t_NsResult>.allocate(capacity:1);r.initialize(to:coax_t_NsResult());defer {r.deinitialize(count:1);r.deallocate()}
            let index=UnsafeMutablePointer<coax_t_MsIndex>.allocate(capacity:1);index.initialize(to:coax_t_MsIndex());defer {index.deinitialize(count:1);index.deallocate()}
            guard NativeAccountCodec.text(raw,{nsr_read($0,coax_v_t_NsResult(data:r,count:1))}),NativeAccountCodec.text(part(r.pointee.payload,Int(r.pointee.n)),{ms_index_read($0,coax_v_t_MsIndex(data:index,count:1))}) else {throw NativeFailure.refused("Invalid outbox")}
            revision=r.pointee.revision
            let pending=NativeAccountCodec.text(relay) {url in NativeAccountCodec.text(original) {nsm_pending(coax_v_t_MsIndex(data:index,count:1),revision,url,$0)}}
            if pending {let event=try await stored("event",identity,revision:revision,payload:id);return NativeAccountCodec.part(event,"payload",cap:65536)}
            cursor=index.pointee.cursor;if cursor==0 {return ""}
        };throw NativeFailure.refused("Outbox incomplete")
    }
    func listHead(_ kind:Int64,identity:String,author:String?=nil)async throws->String {
        let filter=try NativeAccountCodec.write(1024) {out in NativeAccountCodec.text(author ?? identity) {nsm_head_filter($0,kind,out)}}
        let raw=try await stored("index",identity,payload:filter)
        let r=UnsafeMutablePointer<coax_t_NsResult>.allocate(capacity:1);r.initialize(to:coax_t_NsResult());defer {r.deinitialize(count:1);r.deallocate()}
        let index=UnsafeMutablePointer<coax_t_MsIndex>.allocate(capacity:1);index.initialize(to:coax_t_MsIndex());defer {index.deinitialize(count:1);index.deallocate()}
        guard NativeAccountCodec.text(raw,{nsr_read($0,coax_v_t_NsResult(data:r,count:1))}),NativeAccountCodec.text(part(r.pointee.payload,Int(r.pointee.n)),{ms_index_read($0,coax_v_t_MsIndex(data:index,count:1))}) else {throw NativeFailure.refused("Invalid list history")}
        if index.pointee.n==0 {return ""}
        let id=withUnsafeBytes(of:index.pointee.rows) {String(decoding:$0.prefix(64),as:UTF8.self)}
        let event=try await stored("event",identity,revision:r.pointee.revision,payload:id);return NativeAccountCodec.part(event,"payload",cap:65536)
    }
    func canOpen(_ url:String,identity:String)async throws->Bool {
        let settings=try await relaySettings(identity)
        if NativeAccountCodec.text(settings,{s in NativeAccountCodec.text(url) {nsm_saved(s,$0)}}) {return true}
        let head=try await listHead(10050,identity:identity)
        if NativeAccountCodec.text(head,{h in NativeAccountCodec.text(identity) {i in NativeAccountCodec.text(url) {nsm_inbox_target(h,i,$0)}}}) {return true}
        var revision:Int64=0,cursor:Int64=0
        for _ in 0..<4 {
            let raw=try await stored("outbox",identity,revision:revision,cursor:cursor)
            let r=UnsafeMutablePointer<coax_t_NsResult>.allocate(capacity:1);r.initialize(to:coax_t_NsResult());defer {r.deinitialize(count:1);r.deallocate()}
            let index=UnsafeMutablePointer<coax_t_MsIndex>.allocate(capacity:1);index.initialize(to:coax_t_MsIndex());defer {index.deinitialize(count:1);index.deallocate()}
            guard NativeAccountCodec.text(raw,{nsr_read($0,coax_v_t_NsResult(data:r,count:1))}),NativeAccountCodec.text(part(r.pointee.payload,Int(r.pointee.n)),{ms_index_read($0,coax_v_t_MsIndex(data:index,count:1))}) else {throw NativeFailure.refused("Invalid outbox")}
            revision=r.pointee.revision
            if NativeAccountCodec.text(url,{nsm_outbox_target(coax_v_t_MsIndex(data:index,count:1),revision,$0)}) {return true}
            cursor=index.pointee.cursor;if cursor==0 {return false}
        };throw NativeFailure.refused("Outbox incomplete")
    }
    func store(_ raw:String)async throws->[String:Any] {
        let p=UnsafeMutablePointer<coax_t_MsRequest>.allocate(capacity:1);p.initialize(to:coax_t_MsRequest());defer {p.deinitialize(count:1);p.deallocate()}
        guard NativeAccountCodec.text(raw,{nms_request_read($0,coax_v_t_MsRequest(data:p,count:1))}) else {throw NativeFailure.refused("Invalid Nostr store request")}
        let identity=part(p.pointee.identity,64),operation=part(p.pointee.operation,64),action=p.pointee.action,owner=try self.owner()
        _ = try await source(identity,owner)
        let result=try await request("/api/auth/nostr/store/v1alpha1",raw,owner)
        return ["record":try response(result,service:2,action:action,operation:operation,identity:identity)]
    }
    func custody(_ raw:String)async throws->[String:Any] {
        let p=UnsafeMutablePointer<coax_t_NscRequest>.allocate(capacity:1);p.initialize(to:coax_t_NscRequest());defer {nsm_erase(coax_v_u8(data:UnsafeMutableRawPointer(p).assumingMemoryBound(to:UInt8.self),count:Int64(MemoryLayout<coax_t_NscRequest>.size)));p.deinitialize(count:1);p.deallocate()}
        guard NativeAccountCodec.text(raw,{nsc_read($0,coax_v_t_NscRequest(data:p,count:1))}) else {throw NativeFailure.refused("Invalid Nostr custody request")}
        let identity=part(p.pointee.identity,64),operation=part(p.pointee.operation,64),action=p.pointee.action,owner=try self.owner(),source=try await self.source(identity,owner)
        var base=""
        if action==2 || action==5 {guard let admitCustody else {throw NativeFailure.refused("Relay not ready")};base=try await admitCustody(raw);try current(owner)}
        if source==1 {
            let result=try await request("/api/auth/nostr/custody/v1alpha1",raw,owner)
            return ["record":try response(result,service:1,action:action,operation:operation,identity:identity)]
        }
        // A successful public metadata read alone does not authorize local
        // decryption. The account store applies the full human-session gate.
        let probe=try NativeAccountCodec.write(400000) {out in NativeAccountCodec.text("snapshot") {a in NativeAccountCodec.text(operation) {o in NativeAccountCodec.text(identity) {i in NativeAccountCodec.text("") {nms_request_write(a,o,i,$0,coax_v_string(data:nil,count:0),0,0,0,$0,out)}}}}}
        let proof=try await request("/api/auth/nostr/store/v1alpha1",probe,owner)
        _ = try response(proof,service:2,action:1,operation:operation,identity:identity)
        guard NativeAccountCodec.part(proof,"status")=="ok" else {throw NativeFailure.refused("Verify your account first")}
        guard var held=try account.custodyKey(identity) else {throw NativeFailure.refused("This identity’s key is held on another device")}
        defer {held.resetBytes(in:0..<held.count)}
        guard try NativeAccountCodec.publicKey(held).map({String(format:"%02x",$0)}).joined()==identity else {throw NativeFailure.refused("Identity key mismatch")}
        var key=held;defer {key.resetBytes(in:0..<key.count)}
        let digest=try await NativeMessagingWorker.run {()->String in
            let q=UnsafeMutablePointer<coax_t_NscRequest>.allocate(capacity:1);q.initialize(to:coax_t_NscRequest());defer {nsm_erase(coax_v_u8(data:UnsafeMutableRawPointer(q).assumingMemoryBound(to:UInt8.self),count:Int64(MemoryLayout<coax_t_NscRequest>.size)));q.deinitialize(count:1);q.deallocate()}
            guard NativeAccountCodec.text(raw,{nsc_read($0,coax_v_t_NscRequest(data:q,count:1))}) else {throw NativeFailure.refused("Invalid custody request")}
            return try NativeAccountCodec.write(64) {out in NativeAccountCodec.bytes(key) {nso_digest(coax_v_t_NscRequest(data:q,count:1),$0,out) ? 64:-1}}
        };try current(owner)
        func receipt(_ result:String)throws->String {try NativeAccountCodec.write(400000) {out in NativeAccountCodec.text(NativeMessagingServices.actions[Int(action)-1]) {a in NativeAccountCodec.text(operation) {o in NativeAccountCodec.text(identity) {i in NativeAccountCodec.text(digest) {d in NativeAccountCodec.text(result) {nso_request(a,o,i,d,$0,out)}}}}}}}
        if nso_cache_action(action) {
            let existing=try await request("/api/auth/nostr/operation/v1alpha1",try receipt(""),owner)
            _ = try response(existing,service:1,action:action,operation:operation,identity:identity)
            if NativeAccountCodec.part(existing,"status") != "not-ready" {return ["record":existing]}
        }
        if action==3 {
            var inboxes=[String]()
            for i in 0..<11 {
                let peer=try? NativeAccountCodec.write(64) {nsm_peer(coax_v_t_NscRequest(data:p,count:1),Int64(i),$0)}
                guard let peer else {break}
                inboxes.append(try await listHead(10050,identity:identity,author:peer));try current(owner)
            }
            let publicHeads=inboxes
            let candidate=try await NativeMessagingWorker.run {()->String in
                let q=UnsafeMutablePointer<coax_t_NscRequest>.allocate(capacity:1);q.initialize(to:coax_t_NscRequest());defer {nsm_erase(coax_v_u8(data:UnsafeMutableRawPointer(q).assumingMemoryBound(to:UInt8.self),count:Int64(MemoryLayout<coax_t_NscRequest>.size)));q.deinitialize(count:1);q.deallocate()}
                guard NativeAccountCodec.text(raw,{nsc_read($0,coax_v_t_NscRequest(data:q,count:1))}) else {throw NativeFailure.refused("Invalid custody request")}
                let heads=UnsafeMutablePointer<coax_t_NsmHeads>.allocate(capacity:1);heads.initialize(to:coax_t_NsmHeads());defer {heads.deinitialize(count:1);heads.deallocate()}
                for (i,head) in publicHeads.enumerated() {guard NativeAccountCodec.text(head,{nsm_head_put(coax_v_t_NsmHeads(data:heads,count:1),Int64(i),$0)}) else {throw NativeFailure.refused("Inbox unavailable")}}
                let fresh=UnsafeMutablePointer<coax_t_Nip59Fresh>.allocate(capacity:11);fresh.initialize(repeating:coax_t_Nip59Fresh(),count:11)
                defer {nsm_erase(coax_v_u8(data:UnsafeMutableRawPointer(fresh).assumingMemoryBound(to:UInt8.self),count:Int64(11*MemoryLayout<coax_t_Nip59Fresh>.size)));fresh.deinitialize(count:11);fresh.deallocate()}
                guard SecRandomCopyBytes(kSecRandomDefault,11*MemoryLayout<coax_t_Nip59Fresh>.size,fresh)==errSecSuccess else {throw NativeFailure.refused("Entropy unavailable")}
                var facts=coax_t_NsFacts();facts.live=true;facts.human=true;facts.matched=true;facts.approved=true;facts.source=2;facts.provider=2
                return try NativeAccountCodec.write(1048576) {out in NativeAccountCodec.bytes(key) {k in
                    let r=nsm_seal(coax_v_t_NscRequest(data:q,count:1),facts,k,coax_v_t_Nip59Fresh(data:fresh,count:Int64(publicHeads.count)),coax_v_t_NsmHeads(data:heads,count:1),Int64(Date().timeIntervalSince1970),out)
                    return r.status==0 ? r.n:-1
                }}
            };try current(owner)
            let answer=try await request("/api/auth/nostr/send/v1alpha1",candidate,owner)
            return ["record":try response(answer,service:1,action:action,operation:operation,identity:identity)]
        }
        let result=try await NativeMessagingWorker.run {()->(String,String?) in
            let q=UnsafeMutablePointer<coax_t_NscRequest>.allocate(capacity:1);q.initialize(to:coax_t_NscRequest());defer {nsm_erase(coax_v_u8(data:UnsafeMutableRawPointer(q).assumingMemoryBound(to:UInt8.self),count:Int64(MemoryLayout<coax_t_NscRequest>.size)));q.deinitialize(count:1);q.deallocate()}
            guard NativeAccountCodec.text(raw,{nsc_read($0,coax_v_t_NscRequest(data:q,count:1))}) else {throw NativeFailure.refused("Invalid custody request")}
            var fresh=coax_t_Nip59Fresh();guard withUnsafeMutableBytes(of:&fresh,{SecRandomCopyBytes(kSecRandomDefault,$0.count,$0.baseAddress!)})==errSecSuccess else {throw NativeFailure.refused("Entropy unavailable")}
            defer {withUnsafeMutableBytes(of:&fresh) {$0.initializeMemory(as:UInt8.self,repeating:0)}}
            var bytes=[UInt8](repeating:0,count:65536);defer {bytes.withUnsafeMutableBytes {$0.initializeMemory(as:UInt8.self,repeating:0)}}
            var facts=coax_t_NsFacts();facts.live=true;facts.human=true;facts.matched=true;facts.approved=true;facts.source=2;facts.provider=2
            let effect=bytes.withUnsafeMutableBufferPointer {out in NativeAccountCodec.bytes(key) {k in withUnsafeMutablePointer(to:&fresh) {f in NativeAccountCodec.text(base) {nsc_materialize(coax_v_t_NscRequest(data:q,count:1),facts,k,coax_v_t_Nip59Fresh(data:f,count:1),Int64(Date().timeIntervalSince1970),$0,coax_v_u8(data:out.baseAddress,count:65536))}}}}
            let signed=effect.status==0 && effect.cacheable ? String(decoding:bytes.prefix(Int(effect.n)),as:UTF8.self):nil
            let answer=try NativeAccountCodec.write(400000) {out in bytes.withUnsafeMutableBufferPointer {nsc_response(coax_v_t_NscRequest(data:q,count:1),effect,coax_v_u8(data:$0.baseAddress,count:65536),out)}}
            return (answer,signed)
        };try current(owner)
        if action==4 {_ = try await stored("snapshot",identity);try current(owner)}
        let answer:String
        if let signed=result.1 {answer=try await request("/api/auth/nostr/operation/v1alpha1",try receipt(signed),owner)}else {answer=result.0}
        return ["record":try response(answer,service:1,action:action,operation:operation,identity:identity)]
    }
    private static let actions=["capabilities","relay-auth","seal","open","publish-list","group"]
}
