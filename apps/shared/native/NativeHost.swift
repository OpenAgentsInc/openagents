// Firm reason: 6 (UIKit, JavaScriptCore and native account SDK effects).
import UIKit
import AuthenticationServices
import Security

@MainActor final class NativeHost: UIViewController {
    let updates: NativeUpdates
    var epoch = UUID()
    var services = 0
    private var httpEpoch=UUID()
    private var httpTask:Task<Void,Never>?
    var health: Task<Void,Never>?
    var passkey: PasskeyAsk?
    private let auth:AppAuth
    private let wallet=SparkHost()
    private let documents=NativeDocuments()
    let accountServices:NativeAccountServices
    private let relayServices=NativeRelayServices()
    private var relayEpoch=UUID()
    private let surface=NativeControls()
    private lazy var workspace=NativeWorkspace(detail:surface)
    private var dialog:NativeControls?
    private var programs:[NativeProgram]=[]
    private var clock:Timer?
    private var worldLibrary:NativeWorldLibrary?
    init(updates:NativeUpdates,auth:AppAuth = .shared) {self.updates=updates;self.auth=auth;accountServices=NativeAccountServices(auth:auth);super.init(nibName:nil,bundle:nil)}
    required init?(coder:NSCoder) { fatalError() }
    deinit {clock?.invalidate();health?.cancel()}
    override func viewDidLoad() {
        super.viewDidLoad()
        addChild(workspace);workspace.view.frame=view.bounds;workspace.view.autoresizingMask=[.flexibleWidth,.flexibleHeight]
        view.addSubview(workspace.view);workspace.didMove(toParent:self)
        surface.press = { [weak self] id in self?.input(id) }
        surface.edit = { [weak self] id,text in self?.input(id,text) }
        configureSecrets(surface);configureSecrets(workspace.sidebar)
        workspace.press=surface.press;workspace.sidebar.press=surface.press;workspace.sidebar.edit=surface.edit
        clock=Timer.scheduledTimer(withTimeInterval:0.5,repeats:true) { [weak self] _ in
            Task { @MainActor [weak self] in
                guard let self,UIApplication.shared.applicationState == .active,self.view.window != nil else {return}
                do {try self.pump(elapsed:0.5)}catch {self.refuse()}
            }
        }
        load()
    }
    func load() {
        epoch=UUID();relayEpoch=UUID();relayServices.reset();health?.cancel();documents.release();programs=[]
        let directory=updates.boot()
        do {
            programs=try [NativeProgram(directory:directory,module:"app"),NativeProgram(directory:directory,module:"extras")]
            try programs[0].event(Int(cnw_enable_event()),0,"")
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
    private func configureSecrets(_ controls:NativeControls) {controls.secretEdit={ [weak self] id,value in self?.accountServices.editSecret(id,value) };controls.secretBind={ [weak self] field,id in self?.accountServices.bindSecret(field,id:id) }}
    private func refuse() {
        updates.failed()
        surface.render([NativeElement(id:"native.error",role:"notice",label:"The app could not continue. Restart to recover the previous update.")])
    }
    private func pump(elapsed:Double=0)throws {
        guard !programs.isEmpty else {return}
        for p in programs {try p.frame(elapsed:elapsed)}
        let primary=programs[0].elements(),hasNavigation=primary.contains {$0.role=="navigation"}
        let extras=programs.dropFirst().flatMap {$0.elements()}.filter {!hasNavigation || $0.role != "toolbar"}
        workspace.render(primary+extras,history:programs[0].elements(navigation:true))
        if programs[0].nodes.values.contains(where:{$0.id=="worlds.main"}) {
            if worldLibrary==nil {
                let library=try NativeWorldLibrary(directory:updates.current.map(updates.directory) ?? updates.embedded,service:{ [weak self] op,args in guard let self else {throw CancellationError()};return try await self.call(op,args) },navigation:{ [weak self] in self?.input("nav.toggle") }) {}
                worldLibrary=library;workspace.content(library,chrome:library.controls)
            }
        }else if worldLibrary != nil {workspace.content(nil);worldLibrary?.stop();worldLibrary=nil}
        let modal=programs.flatMap {$0.elements(dialog:true)}
        if modal.isEmpty {
            if dialog != nil {accountServices.clear();dialog?.dismiss(animated:true);dialog=nil}
        } else {
            if dialog==nil {
                let vc=NativeControls();vc.sheet=true;vc.press=surface.press;vc.edit=surface.edit;vc.isModalInPresentation=true
                configureSecrets(vc)
                dialog=vc;present(vc,animated:true)
            }
            dialog?.render(modal)
        }
        for p in programs {
            if let slot=p.focus,let id=p.nodes[slot]?.id {
                (dialog?.controls[id] ?? workspace.sidebar.controls[id] ?? surface.controls[id])?.becomeFirstResponder();p.focus=nil
            }
            let (op,body)=try p.action()
            if op==0 {continue}
            let owner=epoch,extra=(p === programs.last)
            let ops=["","auth.state","auth.emailStart","auth.emailVerify","http","auth.logout","updates.check","updates.status","http.cancel","clipboard.write","link.open","environment.commandIDs","environment.wait","world.open","documents.pick","documents.upload","documents.release","","account.service","clipboard.write.v2","relay.read"]
            guard extra || op>0 && op<ops.count else {throw NativeFailure.refused("Unsupported service")}
            var args=(try? JSONSerialization.jsonObject(with:Data(body.utf8))) as? [String:Any] ?? [:]
            if !extra && op==13 {args["nativeWire"]=body}
            if !extra && op==20 {args["nativeWire"]=body;relayEpoch=UUID()}
            if !extra && op==8 {httpEpoch=UUID();httpTask?.cancel();httpTask=nil}
            let requestOwner=httpEpoch
            let relayOwner=relayEpoch
            services += 1
            let task=Task { [weak self,weak p] in
                guard let self else {return}
                defer {self.services -= 1}
                guard let p else {return}
                do {
                    let result=try await self.call(extra ? "wallet.balance":ops[op],args)
                    guard self.epoch==owner,op != 4 || self.httpEpoch==requestOwner,op != 20 || self.relayEpoch==relayOwner else {return}
                    let record=result as? [String:Any] ?? [:]
                    let raw=extra || op==18 || op==20 ? record["record"] as? String ?? "" : op==4 ? record["body"] as? String ?? "" : String(decoding:try JSONSerialization.data(withJSONObject:result),as:UTF8.self)
                    try p.event(extra ? 21:100+op,record["status"] as? Int ?? 0,raw)
                    try self.pump()
                } catch {
                    guard self.epoch==owner,op != 4 || self.httpEpoch==requestOwner,op != 20 || self.relayEpoch==relayOwner else {return}
                    let raw=String(decoding:(try? JSONSerialization.data(withJSONObject:["error":error.localizedDescription])) ?? Data(),as:UTF8.self)
                    do {try p.event(extra ? 21:200+op,0,extra ? "":raw);try self.pump()} catch {self.refuse()}
                }
            }
            if !extra && op==4 {httpTask=task}
        }
    }
    func call(_ op:String,_ b:[String:Any]) async throws -> Any {
        switch op {
        case "relay.read":return try await relayServices.perform(b["nativeWire"] as? String ?? "")
        case "account.service":return try await accountServices.perform(b,presenter:dialog ?? self)
        case "world.open":
            let wire=b["nativeWire"] as? String ?? ""
            let q=UnsafeMutablePointer<coax_t_NwOpen>.allocate(capacity:1);q.initialize(to:coax_t_NwOpen());defer {q.deinitialize(count:1);q.deallocate()}
            let record=coax_v_t_NwOpen(data:q,count:1)
            guard wire.withCString({nwo_read(coax_string(data:$0,count:Int64(wire.utf8.count)),record)}) else {throw NativeFailure.refused("Invalid world request")}
            func part(_ field:Int64)->String {var bytes=[UInt8](repeating:0,count:128);let n=bytes.withUnsafeMutableBufferPointer {nwo_part(record,field,coax_v_u8(data:$0.baseAddress,count:Int64($0.count)))};return n<0 ? "":String(decoding:bytes.prefix(Int(n)),as:UTF8.self)}
            let realm=part(0),path=part(1),edit=q.pointee.edit
            let allowed=realm.withCString {r in path.withCString {p in nwp_open(coax_string(data:r,count:Int64(realm.utf8.count)),coax_string(data:p,count:Int64(path.utf8.count)))}}
            let presenter=b["worldPresenter"] as? UIViewController ?? self
            guard allowed,presenter.presentedViewController==nil else {throw NativeFailure.refused("This world is unavailable")}
            if realm=="catalog" {
                return try await withCheckedThrowingContinuation {continuation in
                    do {
                        let library=try NativeWorldLibrary(directory:updates.current.map(updates.directory) ?? updates.embedded,service:{ [weak self] op,args in guard let self else {throw CancellationError()};return try await self.call(op,args) }) {continuation.resume(returning:["closed":true])}
                        let nav=UINavigationController(rootViewController:library);nav.modalPresentationStyle = .fullScreen;presenter.present(nav,animated:true)
                    }catch {continuation.resume(throwing:error)}
                }
            }
            return await withCheckedContinuation {continuation in
                let world=NativeWorldHost(directory:updates.current.map(updates.directory) ?? updates.embedded,realm:realm,path:path,edit:edit) {continuation.resume(returning:["closed":true])}
                let nav=NativeWorldNavigation(rootViewController:world);nav.modalPresentationStyle = .fullScreen;presenter.present(nav,animated:true)
            }
        case "environment.commandIDs":
            var bytes=[UInt8](repeating:0,count:32)
            guard SecRandomCopyBytes(kSecRandomDefault,bytes.count,&bytes)==errSecSuccess else {throw NativeFailure.refused("Random identities unavailable")}
            return ["command_id":bytes.prefix(16).map {String(format:"%02x",$0)}.joined(),"session_id":bytes.suffix(16).map {String(format:"%02x",$0)}.joined(),"now_ms":Int64(Date().timeIntervalSince1970*1000)]
        case "environment.wait":
            try await Task.sleep(for:.seconds(1));return ["ready":true]
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
        case "documents.pick":
            if let name=b["name"] as? String,let text=b["text"] as? String {return try documents.stage(name:name,data:Data(text.utf8))}
            return try await documents.pick(dialog ?? self)
        case "documents.upload":return try await documents.put(token:b["token"] as? String ?? "",url:b["url"] as? String ?? "")
        case "documents.release":documents.release();return ["released":true]
        case "auth.state": return ["loggedIn":auth.loggedIn,"worldsAvailable":true]
        case "auth.emailStart": return ["pending":try await auth.emailStart(b["email"] as? String ?? "")]
        case "auth.emailVerify": _ = try await auth.emailVerify(code:b["code"] as? String ?? "",pending:b["pending"] as? String ?? ""); return ["loggedIn":true]
        case "auth.keyLogin": _ = try await auth.keyLogin(); return ["loggedIn":true]
        case "auth.passkeyLogin":
            guard let window=view.window, passkey==nil else { throw NativeFailure.refused("Authentication busy") }
            let challenge=try await auth.passkeyChallenge(); let p=PasskeyAsk(anchor:window); passkey=p; defer { passkey=nil }
            _ = try await auth.passkeyLogin(challenge,try await p.assert(challenge)); return ["loggedIn":true]
        case "auth.logout": relayEpoch=UUID();relayServices.reset();accountServices.clear();await auth.logout(); return ["loggedIn":false]
        case "clipboard.write", "clipboard.write.v2":
            guard let text=b["text"] as? String,nt_copy_admit(op == "clipboard.write.v2" ? 2:1,Int64(text.utf8.count)) else {throw NativeFailure.refused("Text is too large")}
            if accountServices.privateClipboard {UIPasteboard.general.setItems([[UIPasteboard.typeAutomatic:text]],options:[.localOnly:true,.expirationDate:Date().addingTimeInterval(60)])}else {UIPasteboard.general.string=text};return ["copied":true]
        case "link.open":
            let raw=b["url"] as? String ?? ""
            if raw=="https://openagents.com/components-gpu" {
                let presenter=presentedViewController ?? self
                return await withCheckedContinuation {continuation in
                    let gallery=NativeComponents {continuation.resume(returning:["opened":true])}
                    let nav=UINavigationController(rootViewController:gallery);nav.modalPresentationStyle = .fullScreen;presenter.present(nav,animated:true)
                }
            }
            guard raw.withCString({nt_link(coax_string(data:$0,count:Int64(raw.utf8.count)))}),let url=URL(string:raw),await UIApplication.shared.open(url) else {throw NativeFailure.refused("This link cannot be opened")}
            return ["opened":true]
        case "http.cancel": return ["cancelled":true]
        case "http":
            let path=b["path"] as? String ?? "", method=b["method"] as? String ?? "GET", body=b["body"] as? String ?? ""
            let allowed=path.withCString { cn_api(coax_string(data:$0,count:Int64(path.utf8.count))) || nwp_account(coax_string(data:$0,count:Int64(path.utf8.count))) }
            guard allowed, ["GET","POST"].contains(method), body.utf8.count<=131072 else { throw NativeFailure.refused("Native request refused") }
            var request=URLRequest(url:URL(string:auth.origin+path)!,timeoutInterval:60); request.httpMethod=method
            if method == "POST" { request.httpBody=Data(body.utf8); request.setValue("application/json",forHTTPHeaderField:"Content-Type");request.setValue(auth.origin,forHTTPHeaderField:"Origin") }
            if auth.loggedIn { request=try await auth.authorize(request) }
            let (stream,response)=try await URLSession.shared.bytes(for:request); var data=Data()
            guard response.url?.host == URL(string:auth.origin)?.host else { throw NativeFailure.refused("Native request redirect refused") }
            for try await byte in stream { guard data.count<1048576 else { throw NativeFailure.refused("Response too large") }; data.append(byte) }
            return ["status":(response as? HTTPURLResponse)?.statusCode ?? 500,"body":String(decoding:data,as:UTF8.self),"contentType":response.mimeType ?? "application/json"]
        default: throw NativeFailure.refused("Unsupported native service")
        }
    }
    func offerActivation() {
        guard presentedViewController==nil else { return }
        let prompt=NativeControls();prompt.sheet=true
        prompt.hostHeader(title:"Update ready",close:"Later") { [weak prompt] in prompt?.dismiss(animated:true) }
        prompt.render([NativeElement(id:"updates.ready",role:"notice",label:"Update ready"),NativeElement(id:"updates.apply",role:"button",label:"Apply update")])
        prompt.press={ [weak self,weak prompt] id in
            guard id=="updates.apply",let self,self.services==0 else{return}
            do {try self.updates.activate();prompt?.dismiss(animated:true) { [weak self] in self?.load() }}catch {}
        }
        present(prompt,animated:true)
    }
}
