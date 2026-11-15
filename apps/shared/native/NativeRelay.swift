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
    override init() {
        queues = .allocate(capacity:6);queues.initialize(repeating:coax_t_NrQueue(),count:6)
        super.init()
        NotificationCenter.default.addObserver(self,selector:#selector(background),name:UIApplication.willResignActiveNotification,object:nil)
    }
    deinit {NotificationCenter.default.removeObserver(self);for pipe in pipes {pipe?.stop()};queues.deinitialize(count:6);queues.deallocate()}
    private func queue(_ slot:Int)->coax_v_t_NrQueue {coax_v_t_NrQueue(data:queues.advanced(by:slot),count:1)}
    func reset() {
        for i in 0..<6 {let old=pipes[i];pipes[i]=nil;old?.stop();records[i]=[];queues[i]=coax_t_NrQueue()}
    }
    @objc private func background() {for i in 0..<6 where queues[i].live {finish(i,"closed")}}
    private func owns(_ slot:Int,_ token:UUID)->Bool {pipes[slot]?.token==token && queues[slot].live}
    private func record(_ slot:Int,_ generation:Int64,_ sequence:Int64,_ kind:String,_ payload:String="")throws->String {
        try NativeAccountCodec.write(400000) {out in NativeAccountCodec.text(kind) {k in NativeAccountCodec.text(payload) {nrr_observation(Int64(slot),generation,sequence,k,$0,out)}}}
    }
    private func append(_ slot:Int,_ kind:String,_ payload:String="") {
        let n=nrq_push(queue(slot),Int64(payload.utf8.count))
        guard n>0 else {finish(slot,"overflow");return}
        records[slot].append(Record(sequence:n,kind:kind,payload:payload))
    }
    private func finish(_ slot:Int,_ kind:String) {
        let old=pipes[slot];pipes[slot]=nil
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
    func perform(_ raw:String)async throws->[String:Any] {
        let q=UnsafeMutablePointer<coax_t_NrrRequest>.allocate(capacity:1);q.initialize(to:coax_t_NrrRequest());defer {q.deinitialize(count:1);q.deallocate()}
        guard NativeAccountCodec.text(raw,{nrr_read($0,coax_v_t_NrrRequest(data:q,count:1))}) else {throw NativeFailure.refused("Relay request refused")}
        let action=Int(q.pointee.action),slot=Int(q.pointee.slot),generation=q.pointee.generation,ack=q.pointee.sequence
        let payload=withUnsafeBytes(of:q.pointee.payload) {String(decoding:$0.prefix(Int(q.pointee.n)),as:UTF8.self)}
        if action==5 {reset();return ["record":""]}
        if action==6 {return ["record":try NativeAccountCodec.write(1024) {nrr_clock_record(Int64(slot),generation,Int64(Date().timeIntervalSince1970),$0)}]}
        if action==1 {
            if queues[slot].generation==generation {return ["record":try poll(slot,generation,queues[slot].ack)]}
            guard nrq_open(queue(slot),generation),let url=URL(string:payload) else {throw NativeFailure.refused("Relay request expired")}
            let old=pipes[slot];pipes[slot]=nil;old?.stop();records[slot]=[]
            let pipe=NativeRelayPipe(),token=pipe.token;pipes[slot]=pipe
            let configuration=URLSessionConfiguration.ephemeral
            configuration.httpCookieStorage=nil;configuration.httpShouldSetCookies=false;configuration.urlCredentialStorage=nil;configuration.urlCache=nil
            let session=URLSession(configuration:configuration,delegate:pipe,delegateQueue:.main)
            let socket=session.webSocketTask(with:url);socket.maximumMessageSize=65536;pipe.session=session;pipe.socket=socket
            pipe.opened={ [weak self] in Task { @MainActor [weak self] in guard let self,self.owns(slot,token) else{return};self.append(slot,"open") }}
            pipe.ended={ [weak self] in Task { @MainActor [weak self] in guard let self,self.owns(slot,token) else{return};self.finish(slot,"closed") }}
            append(slot,"connecting");socket.resume()
            pipe.reader=Task { [weak self,weak pipe] in
                while !Task.isCancelled {
                    do {
                        let message=try await socket.receive()
                        guard let self,let pipe,self.owns(slot,pipe.token) else{return}
                        guard case .string(let text)=message,text.utf8.count<=65536,!text.utf8.contains(0) else {self.finish(slot,"failed");return}
                        self.append(slot,"message",text)
                    }catch {guard let self,let pipe,self.owns(slot,pipe.token) else{return};self.finish(slot,"closed");return}
                }
            }
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
}
