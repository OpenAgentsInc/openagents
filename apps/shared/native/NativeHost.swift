// Firm reason: 6 (UIKit, JavaScriptCore and native account SDK effects).
import UIKit
import AuthenticationServices

@MainActor final class NativePasskey: NSObject, ASAuthorizationControllerDelegate, ASAuthorizationControllerPresentationContextProviding {
    let anchor: UIWindow
    var completion: ((Result<AppAuth.PasskeyAssertion,Error>)->Void)?
    init(_ anchor:UIWindow) { self.anchor=anchor }
    func presentationAnchor(for controller:ASAuthorizationController)->ASPresentationAnchor { anchor }
    func authorizationController(controller:ASAuthorizationController,didCompleteWithAuthorization authorization:ASAuthorization) {
        guard let c=authorization.credential as? ASAuthorizationPlatformPublicKeyCredentialAssertion else { completion?(.failure(NativeFailure.refused("No passkey assertion"))); return }
        completion?(.success(AppAuth.PasskeyAssertion(credentialID:c.credentialID,authenticatorData:c.rawAuthenticatorData,clientData:c.rawClientDataJSON,signature:c.signature,userHandle:c.userID)))
    }
    func authorizationController(controller:ASAuthorizationController,didCompleteWithError error:Error) { completion?(.failure(error)) }
    func get(_ challenge:AppAuth.PasskeyChallenge) async throws -> AppAuth.PasskeyAssertion {
        try await withCheckedThrowingContinuation { continuation in
            completion = { [weak self] result in self?.completion=nil; continuation.resume(with:result) }
            let request=ASAuthorizationPlatformPublicKeyCredentialProvider(relyingPartyIdentifier:challenge.rpID).createCredentialAssertionRequest(challenge:challenge.challenge)
            request.userVerificationPreference = .required
            let c=ASAuthorizationController(authorizationRequests:[request]); c.delegate=self; c.presentationContextProvider=self; c.performRequests()
        }
    }
}

@MainActor final class NativeHost: UIViewController {
    let updates: NativeUpdates
    var epoch = UUID()
    var services = 0
    private var httpEpoch=UUID()
    private var httpTask:Task<Void,Never>?
    var health: Task<Void,Never>?
    var passkey: NativePasskey?
    private let wallet=SparkHost()
    private let surface=NativeControls()
    private var dialog:NativeControls?
    private var programs:[NativeProgram]=[]
    init(updates:NativeUpdates) { self.updates=updates; super.init(nibName:nil,bundle:nil) }
    required init?(coder:NSCoder) { fatalError() }
    override func viewDidLoad() {
        super.viewDidLoad()
        addChild(surface); surface.view.frame=view.bounds; surface.view.autoresizingMask=[.flexibleWidth,.flexibleHeight]
        view.addSubview(surface.view);surface.didMove(toParent:self)
        surface.press = { [weak self] id in self?.input(id) }
        surface.edit = { [weak self] id,text in self?.input(id,text) }
        load()
    }
    func load() {
        epoch=UUID();health?.cancel();programs=[]
        let directory=updates.boot()
        do {
            programs=try [NativeProgram(directory:directory,module:"app"),NativeProgram(directory:directory,module:"extras")]
            try pump()
            Task { [weak self] in _ = try? await self?.call("ready",[:]) }
        } catch {
            if updates.current != nil {updates.failed();load();return}
            surface.render([NativeElement(id:"native.error",role:"notice",label:"The embedded app could not start.")])
        }
    }
    private func input(_ id:String,_ text:String?=nil) {
        do {
            for p in programs where p.nodes.values.contains(where:{$0.id==id}) {try p.input(id,text)}
            try pump()
        } catch {refuse()}
    }
    private func refuse() {
        updates.failed()
        surface.render([NativeElement(id:"native.error",role:"notice",label:"The app could not continue. Restart to recover the previous update.")])
    }
    private func pump()throws {
        for p in programs {try p.frame()}
        surface.render(programs.flatMap {$0.elements()})
        let modal=programs.flatMap {$0.elements(dialog:true)}
        if modal.isEmpty {
            if dialog != nil {dialog?.dismiss(animated:true);dialog=nil}
        } else {
            if dialog==nil {
                let vc=NativeControls();vc.press=surface.press;vc.edit=surface.edit;vc.isModalInPresentation=true
                dialog=vc;present(vc,animated:true)
            }
            dialog?.render(modal)
        }
        for p in programs {
            if let slot=p.focus,let id=p.nodes[slot]?.id {
                (dialog?.controls[id] ?? surface.controls[id])?.becomeFirstResponder();p.focus=nil
            }
            let (op,body)=try p.action()
            if op==0 {continue}
            let owner=epoch,extra=(p === programs.last)
            let ops=["","auth.state","auth.emailStart","auth.emailVerify","http","auth.logout","updates.check","updates.status","http.cancel","clipboard.write","link.open"]
            guard extra || op>0 && op<ops.count else {throw NativeFailure.refused("Unsupported service")}
            let args=(try? JSONSerialization.jsonObject(with:Data(body.utf8))) as? [String:Any] ?? [:]
            if !extra && op==8 {httpEpoch=UUID();httpTask?.cancel();httpTask=nil}
            let requestOwner=httpEpoch
            services += 1
            let task=Task { [weak self,weak p] in
                guard let self else {return}
                defer {self.services -= 1}
                guard let p else {return}
                do {
                    let result=try await self.call(extra ? "wallet.balance":ops[op],args)
                    guard self.epoch==owner,op != 4 || self.httpEpoch==requestOwner else {return}
                    let record=result as? [String:Any] ?? [:]
                    let raw=extra ? record["record"] as? String ?? "" : op==4 ? record["body"] as? String ?? "" : String(decoding:try JSONSerialization.data(withJSONObject:result),as:UTF8.self)
                    try p.event(extra ? 21:100+op,record["status"] as? Int ?? 0,raw)
                    try self.pump()
                } catch {
                    guard self.epoch==owner,op != 4 || self.httpEpoch==requestOwner else {return}
                    let raw=String(decoding:(try? JSONSerialization.data(withJSONObject:["error":error.localizedDescription])) ?? Data(),as:UTF8.self)
                    do {try p.event(extra ? 21:200+op,0,extra ? "":raw);try self.pump()} catch {self.refuse()}
                }
            }
            if !extra && op==4 {httpTask=task}
        }
    }
    func call(_ op:String,_ b:[String:Any]) async throws -> Any {
        switch op {
        case "ready":
            let owner=epoch
            health=Task { [weak self] in
                var ticks=0
                while ticks<30 { try? await Task.sleep(for:.seconds(1)); guard !Task.isCancelled,let self,self.epoch==owner else { return }; if UIApplication.shared.applicationState == .active { ticks += 1 } }
                self?.updates.healthy()
            }
            Task { [weak self] in
                guard let self else { return }
                _ = try? await self.updates.check()
                if self.updates.pending != nil {self.offerActivation()}
            }
            return ["message":updates.details()]
        case "updates.status": return ["message":updates.details()]
        case "updates.check":
            let text=try await updates.check()
            if updates.pending != nil || updates.read("pending.cnu") != nil { offerActivation() }
            return ["message":text]
        case "wallet.balance":
            let observation:SparkHost.Observation
            #if DEBUG
            // Simulator UI fixture only. No seed is created or read by this path.
            if UserDefaults.standard.bool(forKey:"OAAutomation"),let fixture=UserDefaults.standard.string(forKey:"OABalanceFixture") {
                observation=fixture=="missing" ? .init(status:2):.init(status:1,sats:UInt64(fixture) ?? UInt64.max)
            } else {observation=await wallet.balance()}
            #else
            observation=await wallet.balance()
            #endif
            var out=[UInt8](repeating:0,count:256)
            let n=out.withUnsafeMutableBufferPointer {wb_encode(observation.status,observation.sats,coax_v_u8(data:$0.baseAddress,count:Int64($0.count)))}
            guard n>0 else {throw NativeFailure.refused("Balance unavailable")}
            return ["record":String(decoding:out.prefix(Int(n)),as:UTF8.self)]
        case "auth.state": return ["loggedIn":AppAuth.shared.loggedIn]
        case "auth.emailStart": return ["pending":try await AppAuth.shared.emailStart(b["email"] as? String ?? "")]
        case "auth.emailVerify": _ = try await AppAuth.shared.emailVerify(code:b["code"] as? String ?? "",pending:b["pending"] as? String ?? ""); return ["loggedIn":true]
        case "auth.keyLogin": _ = try await AppAuth.shared.keyLogin(); return ["loggedIn":true]
        case "auth.passkeyLogin":
            guard let window=view.window, passkey==nil else { throw NativeFailure.refused("Authentication busy") }
            let challenge=try await AppAuth.shared.passkeyChallenge(); let p=NativePasskey(window); passkey=p; defer { passkey=nil }
            _ = try await AppAuth.shared.passkeyLogin(challenge,try await p.get(challenge)); return ["loggedIn":true]
        case "auth.logout": await AppAuth.shared.logout(); return ["loggedIn":false]
        case "clipboard.write":
            guard let text=b["text"] as? String,text.utf8.count<=65536 else {throw NativeFailure.refused("Text is too large")}
            UIPasteboard.general.string=text;return ["copied":true]
        case "link.open":
            let raw=b["url"] as? String ?? ""
            guard raw.withCString({nt_link(coax_string(data:$0,count:Int64(raw.utf8.count)))}),let url=URL(string:raw),await UIApplication.shared.open(url) else {throw NativeFailure.refused("This link cannot be opened")}
            return ["opened":true]
        case "http.cancel": return ["cancelled":true]
        case "http":
            let path=b["path"] as? String ?? "", method=b["method"] as? String ?? "GET", body=b["body"] as? String ?? ""
            let allowed=path.withCString { cn_api(coax_string(data:$0,count:Int64(path.utf8.count))) }
            guard allowed, ["GET","POST"].contains(method), body.utf8.count<=65536 else { throw NativeFailure.refused("Native request refused") }
            var request=URLRequest(url:URL(string:AppAuth.defaultOrigin+path)!,timeoutInterval:60); request.httpMethod=method
            if method == "POST" { request.httpBody=Data(body.utf8); request.setValue("application/json",forHTTPHeaderField:"Content-Type") }
            if AppAuth.shared.loggedIn { request=try await AppAuth.shared.authorize(request) }
            let (stream,response)=try await URLSession.shared.bytes(for:request); var data=Data()
            guard response.url?.host == URL(string:AppAuth.defaultOrigin)?.host else { throw NativeFailure.refused("Native request redirect refused") }
            for try await byte in stream { guard data.count<1048576 else { throw NativeFailure.refused("Response too large") }; data.append(byte) }
            return ["status":(response as? HTTPURLResponse)?.statusCode ?? 500,"body":String(decoding:data,as:UTF8.self),"contentType":response.mimeType ?? "application/json"]
        default: throw NativeFailure.refused("Unsupported native service")
        }
    }
    func offerActivation() {
        guard presentedViewController==nil else { return }
        let alert=UIAlertController(title:"Update ready",message:"A signed Coax Native update is downloaded. Restart this view to use it.",preferredStyle:.alert)
        alert.addAction(UIAlertAction(title:"Later",style:.cancel))
        alert.addAction(UIAlertAction(title:"Apply update",style:.default) { [weak self] _ in
            guard let self,self.services==0 else { return }
            do { try self.updates.activate(); self.load() } catch { }
        }); present(alert,animated:true)
    }
}
