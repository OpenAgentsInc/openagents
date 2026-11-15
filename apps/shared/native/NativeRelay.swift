// Firm reason: 6 (URLSession WebSocket, application lifecycle and wall-clock SDK effects).
// Coax admits every request and owns the retained-queue ledger. These sockets
// carry public REQ/CLOSE frames; no account credentials or signing key.
import UIKit

private final class NativeRelayPipe:NSObject,URLSessionWebSocketDelegate {
    let token=UUID()
    var socket:URLSessionWebSocketTask?
    var session:URLSession?
    var reader:Task<Void,Never>?
    var opened:(()->Void)?
    var ended:(()->Void)?
    func urlSession(_ session:URLSession,webSocketTask:URLSessionWebSocketTask,didOpenWithProtocol protocol:String?) {opened?()}
    func urlSession(_ session:URLSession,webSocketTask:URLSessionWebSocketTask,didCloseWith closeCode:URLSessionWebSocketTask.CloseCode,reason:Data?) {ended?()}
    func urlSession(_ session:URLSession,task:URLSessionTask,didCompleteWithError error:Error?) {if error != nil {ended?()}}
    func urlSession(_ session:URLSession,task:URLSessionTask,willPerformHTTPRedirection response:HTTPURLResponse,newRequest request:URLRequest,completionHandler:@escaping(URLRequest?)->Void) {completionHandler(nil);ended?()}
    func stop() {reader?.cancel();reader=nil;socket?.cancel(with:.goingAway,reason:nil);session?.invalidateAndCancel();socket=nil;session=nil}
}

@MainActor final class NativeRelayServices:NSObject {
    private struct Record {let sequence:Int64;let kind:String;let payload:String}
    private var pipes=[NativeRelayPipe?](repeating:nil,count:6)
    private var records=[[Record]](repeating:[],count:6)
    private let queues:UnsafeMutablePointer<coax_t_NrQueue>
    private let owners:UnsafeMutablePointer<coax_t_NrsOwner>
    private let auth:AppAuth
    weak var messaging:NativeMessagingServices?
    private var sessionMode=[Bool](repeating:false,count:6)
    private var logins=[UUID?](repeating:nil,count:6)
    private var identities=[String](repeating:"",count:6),urls=[String](repeating:"",count:6)
    init(auth:AppAuth = .shared) {
        self.auth=auth;owners = .allocate(capacity:6);owners.initialize(repeating:coax_t_NrsOwner(),count:6)
        queues = .allocate(capacity:6);queues.initialize(repeating:coax_t_NrQueue(),count:6)
        super.init()
        NotificationCenter.default.addObserver(self,selector:#selector(background),name:UIApplication.willResignActiveNotification,object:nil)
        NotificationCenter.default.addObserver(self,selector:#selector(sessionChanged),name:AppAuth.didChangeSession,object:auth)
    }
    deinit {NotificationCenter.default.removeObserver(self);for pipe in pipes {pipe?.stop()};queues.deinitialize(count:6);queues.deallocate();owners.deinitialize(count:6);owners.deallocate()}
    private func queue(_ slot:Int)->coax_v_t_NrQueue {coax_v_t_NrQueue(data:queues.advanced(by:slot),count:1)}
    private func owner(_ slot:Int)->coax_v_t_NrsOwner {coax_v_t_NrsOwner(data:owners.advanced(by:slot),count:1)}
    private func resetSlot(_ i:Int) {let old=pipes[i];pipes[i]=nil;old?.stop();records[i]=[];queues[i]=coax_t_NrQueue();nrs_retire(owner(i));sessionMode[i]=false;logins[i]=nil;identities[i]="";urls[i]=""}
    func reset(readOnly:Bool=false) {
        for i in 0..<6 where !readOnly || !sessionMode[i] {resetSlot(i)}
    }
    @objc private func background() {for i in 0..<6 where queues[i].live {finish(i,"closed")}}
    @objc private func sessionChanged() {Task { @MainActor [weak self] in guard let self else {return};for i in 0..<6 where self.sessionMode[i] && self.logins[i] != self.auth.sessionOwner {self.resetSlot(i)}}}
    private func owns(_ slot:Int,_ token:UUID)->Bool {pipes[slot]?.token==token && queues[slot].live && (!sessionMode[slot] || logins[slot]==auth.sessionOwner)}
    private func record(_ slot:Int,_ generation:Int64,_ sequence:Int64,_ kind:String,_ payload:String="")throws->String {
        try NativeAccountCodec.write(sessionMode[slot] ? 800000:400000) {out in NativeAccountCodec.text(kind) {k in NativeAccountCodec.text(payload) {p in
            if sessionMode[slot] {return NativeAccountCodec.text(identities[slot]) {nrs_observation(Int64(slot),generation,sequence,$0,k,p,out)}}
            return nrr_observation(Int64(slot),generation,sequence,k,p,out)
        }}}
    }
    private func append(_ slot:Int,_ kind:String,_ payload:String="") {
        let n=nrq_push_bounded(queue(slot),Int64(payload.utf8.count),sessionMode[slot] ? 131072:65536)
        guard n>0 else {finish(slot,"overflow");return}
        records[slot].append(Record(sequence:n,kind:kind,payload:payload))
    }
    private func finish(_ slot:Int,_ kind:String) {
        let old=pipes[slot];pipes[slot]=nil
        nrs_retire(owner(slot))
        let n=nrq_terminal(queue(slot),0)
        if n>0 {records[slot].append(Record(sequence:n,kind:kind,payload:""))}
        old?.stop()
    }
    private func poll(_ slot:Int,_ generation:Int64,_ ack:Int64)throws->String {
        guard queues[slot].generation==generation else {throw NativeFailure.refused("Relay request expired")}
        let next=nrq_poll(queue(slot),ack)
        guard next>=0 else {throw NativeFailure.refused("Relay acknowledgement refused")}
        records[slot].removeAll {$0.sequence<=ack}
        guard next>0 else {return try record(slot,generation,ack,"idle")}
        guard let first=records[slot].first,first.sequence==next else {throw NativeFailure.refused("Relay observation unavailable")}
        return try record(slot,generation,next,first.kind,first.payload)
    }
    private func connect(_ slot:Int,_ url:URL) {
        let old=pipes[slot];pipes[slot]=nil;old?.stop();records[slot]=[]
        let pipe=NativeRelayPipe(),token=pipe.token;pipes[slot]=pipe
        let configuration=URLSessionConfiguration.ephemeral
        configuration.httpCookieStorage=nil;configuration.httpShouldSetCookies=false;configuration.urlCredentialStorage=nil;configuration.urlCache=nil
        let session=URLSession(configuration:configuration,delegate:pipe,delegateQueue:.main)
        let socket=session.webSocketTask(with:url);socket.maximumMessageSize=sessionMode[slot] ? 131072:65536;pipe.session=session;pipe.socket=socket
        pipe.opened={ [weak self] in Task { @MainActor [weak self] in guard let self,self.owns(slot,token) else{return};self.append(slot,"open") }}
        pipe.ended={ [weak self] in Task { @MainActor [weak self] in guard let self,self.owns(slot,token) else{return};self.finish(slot,"closed") }}
        append(slot,"connecting");socket.resume()
        pipe.reader=Task { [weak self,weak pipe] in
            while !Task.isCancelled {
                do {
                    let message=try await socket.receive()
                    guard let self,let pipe,self.owns(slot,pipe.token) else{return}
                    guard case .string(let text)=message,text.utf8.count<=(self.sessionMode[slot] ? 131072:65536),!text.utf8.contains(0) else {self.finish(slot,"failed");return}
                    if self.sessionMode[slot] {
                        guard let messaging=self.messaging else {self.finish(slot,"failed");return}
                        try await messaging.received(text,identity:self.identities[slot],relay:self.urls[slot])
                        guard self.owns(slot,pipe.token) else {return}
                        _ = NativeAccountCodec.text(text) {nrs_input(self.owner(slot),self.queues[slot].generation,$0)}
                    }
                    self.append(slot,"message",text)
                }catch {guard let self,let pipe,self.owns(slot,pipe.token) else{return};self.finish(slot,"closed");return}
            }
        }
    }
    func perform(_ raw:String)async throws->[String:Any] {
        let q=UnsafeMutablePointer<coax_t_NrrRequest>.allocate(capacity:1);q.initialize(to:coax_t_NrrRequest());defer {q.deinitialize(count:1);q.deallocate()}
        guard NativeAccountCodec.text(raw,{nrr_read($0,coax_v_t_NrrRequest(data:q,count:1))}) else {throw NativeFailure.refused("Relay request refused")}
        let action=Int(q.pointee.action),slot=Int(q.pointee.slot),generation=q.pointee.generation,ack=q.pointee.sequence
        let payload=withUnsafeBytes(of:q.pointee.payload) {String(decoding:$0.prefix(Int(q.pointee.n)),as:UTF8.self)}
        if action==5 {reset(readOnly:true);return ["record":""]}
        if action==6 {return ["record":try NativeAccountCodec.write(1024) {nrr_clock_record(Int64(slot),generation,Int64(Date().timeIntervalSince1970),$0)}]}
        guard !sessionMode[slot] else {throw NativeFailure.refused("Relay slot belongs to an authenticated session")}
        if action==1 {
            if queues[slot].generation==generation {return ["record":try poll(slot,generation,queues[slot].ack)]}
            guard nrq_open(queue(slot),generation),let url=URL(string:payload) else {throw NativeFailure.refused("Relay request expired")}
            urls[slot]=payload;connect(slot,url)
            return ["record":try poll(slot,generation,0)]
        }
        guard queues[slot].generation==generation else {throw NativeFailure.refused("Relay request expired")}
        if action==3 {return ["record":try poll(slot,generation,ack)]}
        if action==4 {_ = try poll(slot,generation,ack);finish(slot,"closed");return ["record":try record(slot,generation,ack,"closed")]}
        guard action==2,queues[slot].live,let pipe=pipes[slot],let socket=pipe.socket else {throw NativeFailure.refused("Relay disconnected")}
        let token=pipe.token
        do {try await socket.send(.string(payload))}catch {if owns(slot,token) {finish(slot,"closed")};throw NativeFailure.refused("Relay disconnected")}
        guard owns(slot,token) else {throw CancellationError()}
        return ["record":try record(slot,generation,queues[slot].ack,"idle")]
    }
    private func authState<T>(_ slot:Int,_ body:(coax_v_t_Nip42)->T)->T {withUnsafeMutableBytes(of:&owners[slot].auth) {body(coax_v_t_Nip42(data:$0.baseAddress!.assumingMemoryBound(to:coax_t_Nip42.self),count:1))}}
    func performSession(_ raw:String)async throws->[String:Any] {
        let q=UnsafeMutablePointer<coax_t_NrsRequest>.allocate(capacity:1);q.initialize(to:coax_t_NrsRequest());defer {q.deinitialize(count:1);q.deallocate()}
        guard NativeAccountCodec.text(raw,{nrs_read($0,coax_v_t_NrsRequest(data:q,count:1))}),auth.loggedIn,let messaging else {throw NativeFailure.refused("Session unavailable")}
        let action=Int(q.pointee.action),slot=Int(q.pointee.slot),generation=q.pointee.generation,ack=q.pointee.sequence,login=auth.sessionOwner
        let identity=withUnsafeBytes(of:q.pointee.identity) {String(decoding:$0.prefix(64),as:UTF8.self)},payload=withUnsafeBytes(of:q.pointee.payload) {String(decoding:$0.prefix(Int(q.pointee.n)),as:UTF8.self)}
        if action==5 {for i in 0..<6 where sessionMode[i] && identities[i]==identity && logins[i]==login {resetSlot(i)};return ["record":""]}
        if action==6 {return ["record":try NativeAccountCodec.write(1024) {out in NativeAccountCodec.text(identity) {i in NativeAccountCodec.text("clock") {k in NativeAccountCodec.text(String(Int64(Date().timeIntervalSince1970))) {nrs_observation(Int64(slot),generation,0,i,k,$0,out)}}}}]}
        if action==1 {
            let permitted=try await messaging.canOpen(payload,identity:identity)
            guard login==auth.sessionOwner,permitted,let url=URL(string:payload) else {throw NativeFailure.refused("Relay is not selected")}
            if queues[slot].generation==generation {
                guard sessionMode[slot],identities[slot]==identity,urls[slot]==payload,logins[slot]==login else {throw NativeFailure.refused("Relay request expired")}
                return ["record":try poll(slot,generation,queues[slot].ack)]
            }
            guard nrq_open(queue(slot),generation),NativeAccountCodec.text(identity,{i in NativeAccountCodec.text(payload) {nrs_bind(owner(slot),generation,i,$0)}}) else {throw NativeFailure.refused("Relay request expired")}
            sessionMode[slot]=true;logins[slot]=login;identities[slot]=identity;urls[slot]=payload;connect(slot,url)
            return ["record":try poll(slot,generation,0)]
        }
        guard sessionMode[slot],logins[slot]==login,identities[slot]==identity,queues[slot].generation==generation else {throw NativeFailure.refused("Relay request expired")}
        if action==3 {return ["record":try poll(slot,generation,ack)]}
        if action==4 {_ = try poll(slot,generation,ack);finish(slot,"closed");return ["record":try record(slot,generation,ack,"closed")]}
        guard action==2,let pipe=pipes[slot],let socket=pipe.socket,owns(slot,pipe.token) else {throw NativeFailure.refused("Relay disconnected")};let token=pipe.token
        func allowed(_ committed:String,_ pending:Bool)->Int64 {NativeAccountCodec.text(payload) {f in NativeAccountCodec.text(identity) {i in NativeAccountCodec.text(committed) {c in authState(slot) {nrs_send(f,i,owns(slot,token),c,pending,$0,generation,Int64(Date().timeIntervalSince1970))}}}}}
        if allowed("",false)==0 {
            let original=NativeAccountCodec.text(payload) {f in let original=nrs_original(f);return String(decoding:UnsafeRawBufferPointer(start:original.data,count:Int(original.count)),as:UTF8.self)}
            guard !original.isEmpty else {throw NativeFailure.refused("Publish refused")}
            let committed=try await messaging.staged(original,identity:identity,relay:urls[slot]);guard owns(slot,token),allowed(committed,!committed.isEmpty)==2 else {throw NativeFailure.refused("Publish is not pending in the durable outbox")}
        }
        _ = NativeAccountCodec.text(payload) {nrs_watch(owner(slot),generation,$0)}
        do {try await socket.send(.string(payload))}catch {if owns(slot,token) {finish(slot,"closed")};throw NativeFailure.refused("Relay disconnected")}
        guard owns(slot,token) else {throw CancellationError()};return ["record":try record(slot,generation,queues[slot].ack,"idle")]
    }
    func authorizeCustody(_ raw:String)async throws->String {
        let q=UnsafeMutablePointer<coax_t_NscRequest>.allocate(capacity:1);q.initialize(to:coax_t_NscRequest());defer {nsm_erase(coax_v_u8(data:UnsafeMutableRawPointer(q).assumingMemoryBound(to:UInt8.self),count:Int64(MemoryLayout<coax_t_NscRequest>.size)));q.deinitialize(count:1);q.deallocate()}
        let p=UnsafeMutablePointer<coax_t_NscPurpose>.allocate(capacity:1);p.initialize(to:coax_t_NscPurpose());defer {nsm_erase(coax_v_u8(data:UnsafeMutableRawPointer(p).assumingMemoryBound(to:UInt8.self),count:Int64(MemoryLayout<coax_t_NscPurpose>.size)));p.deinitialize(count:1);p.deallocate()}
        guard NativeAccountCodec.text(raw,{nsc_read($0,coax_v_t_NscRequest(data:q,count:1))}) else {throw NativeFailure.refused("Custody refused")}
        let identity=withUnsafeBytes(of:q.pointee.identity) {String(decoding:$0.prefix(64),as:UTF8.self)},purpose=withUnsafeBytes(of:q.pointee.payload) {String(decoding:$0.prefix(Int(q.pointee.n)),as:UTF8.self)},login=auth.sessionOwner
        guard NativeAccountCodec.text(identity,{i in NativeAccountCodec.text(purpose) {nsp_read(q.pointee.action,i,$0,coax_v_t_NscPurpose(data:p,count:1))}}) else {throw NativeFailure.refused("Custody refused")}
        if q.pointee.action==2 {
            let url=withUnsafeBytes(of:p.pointee.url) {String(decoding:$0.prefix(Int(p.pointee.nu)),as:UTF8.self)}
            for i in 0..<6 where sessionMode[i] && logins[i]==login && identities[i]==identity && urls[i]==url && queues[i].live {
                if authState(i,{nsc_auth_current(coax_v_t_NscPurpose(data:p,count:1),$0)}) {return ""}
            };throw NativeFailure.refused("Relay challenge expired")
        }
        guard q.pointee.action==5,let messaging else {throw NativeFailure.refused("Purpose unavailable")}
        let settings=try await messaging.relaySettings(identity);guard login==auth.sessionOwner else {throw CancellationError()};var required=0
        for i in 0..<6 {
            let url=NativeAccountCodec.part(settings,"relays[\(i)]");if url.isEmpty {break};required += 1
            guard (0..<6).contains(where:{slot in sessionMode[slot] && logins[slot]==login && identities[slot]==identity && urls[slot]==url && NativeAccountCodec.text(identity,{nrs_read_ready(owner(slot),queues[slot].generation,$0,p.pointee.kind)})}) else {throw NativeFailure.refused("Relay list read is incomplete")}
        }
        guard required>0 else {throw NativeFailure.refused("No relay selected")};return try await messaging.listHead(p.pointee.kind,identity:identity)
    }
}
