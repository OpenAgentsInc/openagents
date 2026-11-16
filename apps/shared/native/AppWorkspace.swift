// Firm reason: 6 (protected pending storage and authenticated HTTP/device transport).
// Coax owns all request and reply validation. Store before executing any write.
import Foundation
@MainActor final class AppWorkspaceNative:AppProviderService {
    private let auth:AppAuth
    private let devices:AppDeviceService
    private let store:AppIdentityPendingStore
    private let http:AppHTTP
    private var running=false
    private var serial:UInt64=0
    init(auth:AppAuth,devices:AppDeviceService,store:AppIdentityPendingStore?=nil,protocols:[AnyClass]=[]) {
        self.auth=auth;self.devices=devices
        #if os(iOS)
        self.store=store ?? KeychainIdentityPending(service:"com.openagents.app.workspace.pending.v1")
        #else
        self.store=store ?? AppFileIdentityPending(folder:AppFileViewStore.folder.appendingPathComponent("pending-workspace-v1"),limit:24576)
        #endif
        http=AppHTTP(policy:.init(origin:auth.origin,admit:{method,path,bytes in method=="POST" && path.hasPrefix("/api/chat/environment/workspace/v1?host=") && bytes<=24576}),authorize:AppAccount.authorizer(auth),protocols:protocols)
    }
    func clear() {}
    func perform(mode:Int32,host:String,metadata:Data,done:@escaping(AppCompletion.State,Int32,Data)->Void) {
        guard !running,metadata.withUnsafeBytes({oc_workspace_effect(mode,host,$0.bindMemory(to:UInt8.self).baseAddress,Int32(metadata.count))}) != 0 else {done(.failed,400,Data());return}
        let viaAccount=mode==1 || mode==3
        guard !viaAccount || auth.loggedIn else {done(.failed,401,Data());return}
        let owner=auth.sessionOwner,scope=(viaAccount ? "account:"+(auth.tokens?.account ?? ""):"paired")+":"+host
        if mode>=2 {done(.done,200,store.load(scope) ?? Data());return}
        let mutation=metadata.withUnsafeBytes {oc_workspace_mutation($0.bindMemory(to:UInt8.self).baseAddress,Int32(metadata.count))}
        let kept=store.load(scope)
        if let kept,kept != metadata {done(.unknown,409,Data());return}
        if mutation==1 && kept==nil {guard store.save(metadata,account:scope),store.load(scope)==metadata else {done(.failed,507,Data());return}}
        running=true
        func answer(_ state:AppCompletion.State,_ status:Int32,_ raw:Data) {
            self.running=false
            guard !viaAccount || self.auth.sessionOwner==owner else {done(.cancelled,0,Data());return}
            let valid=metadata.withUnsafeBytes {q in raw.withUnsafeBytes {v in oc_workspace_reply(host,q.bindMemory(to:UInt8.self).baseAddress,Int32(metadata.count),v.bindMemory(to:UInt8.self).baseAddress,Int32(raw.count))}} != 0
            if state == .done,status==200,valid {
                if mutation==1 {self.store.remove(scope);guard self.store.load(scope)==nil else {done(.unknown,507,raw);return}}
                done(.done,200,raw)
            } else {done(mutation==1 ? .unknown:state == .done ? .failed:state,status,Data())}
        }
        if viaAccount {serial += 1;http.start(op:serial,generation:1,.init(method:"POST",path:"/api/chat/environment/workspace/v1?host="+host,body:metadata,limit:1048576,timeoutMs:55000)) {outcome in
            switch outcome {case .answer(let status,let raw,_):answer(.done,Int32(status),raw);case .failed:answer(.failed,0,Data());case .unknown:answer(.unknown,0,Data());case .part:break}
        }} else {devices.call(host:host,body:metadata) {state,raw in answer(state,200,raw)}}
    }
}
