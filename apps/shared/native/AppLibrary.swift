// Firm reason: 6 (authenticated HTTP, platform clock and durable preferences; Coax owns merge and wire bytes).
import Foundation
@MainActor final class AppLibraryNative {
    private let auth:AppAuth
    private let http:AppHTTP
    private var timer:Timer?
    private var serial:UInt64=0
    private var running=false
    private var epoch=UUID()
    var request:()->Data? = {nil}
    var persist:()->Void = {}
    var answer:(Int32,Data)->Void = {_,_ in}
    init(auth:AppAuth) {
        self.auth=auth
        http=AppHTTP(policy:.init(origin:auth.origin,admit:{method,path,bytes in path=="/api/chat/library/v1" && (method=="GET" && bytes==0 || method=="POST" && bytes<=131072)}),authorize:AppAccount.authorizer(auth))
    }
    func reset() {epoch=UUID();running=false;timer?.invalidate();timer=nil}
    func start() {
        guard timer==nil else {return}
        timer=Timer.scheduledTimer(withTimeInterval:15,repeats:true) {[weak self] _ in Task {@MainActor in self?.sync()}}
        sync()
    }
    func sync() {
        guard auth.loggedIn,!running,let body=request() else {return}
        persist() // Exact pending bytes reach durable storage before HTTP starts.
        let scope=epoch,owner=auth.sessionOwner
        serial += 1;running=true
        http.start(op:serial,generation:1,.init(method:body.isEmpty ? "GET":"POST",path:"/api/chat/library/v1",body:body,limit:131072,timeoutMs:20000)) {[weak self] outcome in
            guard let self,self.epoch==scope,self.auth.sessionOwner==owner else {return}
            switch outcome {
            case .answer(let status,let raw,_): self.running=false;self.answer(Int32(status),raw);self.persist()
            case .failed,.unknown:self.running=false
            case .part:break
            }
        }
    }
}

/// One platform coordinator for iOS and macOS. It caches the login partition;
/// ordinary frames compare a scalar and never read Keychain or call HTTP.
@MainActor final class AppChatLibraryNative {
    private let auth:AppAuth
    private let owner:UnsafeMutableRawPointer
    private let transport:AppLibraryNative
    private var partition:String?
    private var needsRestore=true
    private var revision:Int32 = -1
    private let wake:()->Void
    init(auth:AppAuth,owner:UnsafeMutableRawPointer,wake:@escaping()->Void) {
        self.auth=auth;self.owner=owner;self.wake=wake;transport=AppLibraryNative(auth:auth)
        transport.request={ [weak self] in
            guard let self,self.partition?.hasPrefix("account:")==true else {return nil}
            var raw=[CChar](repeating:0,count:131073)
            let n=oc_library_sync_request(self.owner,UUID().uuidString.replacingOccurrences(of:"-",with:"").lowercased(),&raw,Int32(raw.count))
            return n>=0 ? Data(raw.prefix(Int(n)).map {UInt8(bitPattern:$0)}):nil
        }
        transport.persist={ [weak self] in self?.persist() }
        transport.answer={ [weak self] status,data in
            guard let self,let raw=String(data:data,encoding:.utf8) else {return}
            _=oc_library_sync_answer(self.owner,status,raw);self.persist();self.wake()
        }
    }
    func accountChanged() {transport.reset();needsRestore=true;wake()}
    func background() {transport.reset();persist()}
    func restore() {
        guard needsRestore else {return};needsRestore=false
        let next=auth.tokens.map {$0.account.isEmpty ? "unidentified:"+auth.sessionOwner.uuidString:"account:"+$0.account} ?? "guest"
        guard next != partition else {if next.hasPrefix("account:") {transport.start()};return}
        transport.reset();partition=next
        #if DEBUG
        if next=="guest",let encoded=ProcessInfo.processInfo.environment["OA_CHAT_LIBRARY_FIXTURE"],let data=Data(base64Encoded:encoded),let text=String(data:data,encoding:.utf8) {UserDefaults.standard.set(text,forKey:"openagents.chat-library."+next)}
        #endif
        let raw=UserDefaults.standard.string(forKey:"openagents.chat-library."+next) ?? ""
        _=oc_library_restore(owner,raw)
        _=oc_library_sync_restore(owner,next.hasPrefix("account:") ? UserDefaults.standard.string(forKey:"openagents.chat-library.sync."+next) ?? "":"")
        revision=oc_library_revision(owner);if next.hasPrefix("account:") {transport.start()}
    }
    func frame() {
        let current=oc_library_revision(owner);guard current != revision,let partition else {return}
        var bytes=[CChar](repeating:0,count:49153)
        guard oc_library_save(owner,&bytes,Int32(bytes.count))>=0 else {return}
        UserDefaults.standard.set(String(cString:bytes),forKey:"openagents.chat-library."+partition)
        revision=current;persist();transport.sync()
    }
    private func persist() {
        guard let partition,partition.hasPrefix("account:") else {return}
        var bytes=[CChar](repeating:0,count:524289)
        guard oc_library_sync_save(owner,&bytes,Int32(bytes.count))>0 else {return}
        UserDefaults.standard.set(String(cString:bytes),forKey:"openagents.chat-library.sync."+partition)
    }
}
