// Firm reason: 6 (UIKit/Metal world lifecycle, touch projection and account SDK effects).
import UIKit
import MetalKit
import CryptoKit

// The C host can cancel before its request reaches the main actor. Hold that
// cancellation until the URLSession task has been installed.
private final class NativeWorldCancellation:@unchecked Sendable {
    private let lock=NSLock();private var ended=false,work:(()->Void)?
    func bind(_ work:@escaping()->Void) {lock.lock();let ended=ended;if !ended {self.work=work};lock.unlock();if ended {work()}}
    func cancel() {lock.lock();ended=true;let work=work;self.work=nil;lock.unlock();work?()}
}

@MainActor final class NativeWorldNetwork {
    private let transport=NativeWorldSession()
    private var jobs:[UUID:Task<Void,Never>]=[:]
    private var ended=false
    private(set) var player=""
    func ready()async {
        let account=AppAuth.shared.tokens?.account ?? "guest"
        let name=NativePackage.hex(Data(SHA256.hash(data:Data(account.utf8))))
        let file=URL(fileURLWithPath:NSHomeDirectory()).appendingPathComponent(".openagents/worlds/players/\(name).json")
        if let raw=try? Data(contentsOf:file),let text=String(data:raw,encoding:.utf8) {player=text}
        do {
            if player.isEmpty {
                var q=URLRequest(url:URL(string:AppAuth.defaultOrigin+"/api/playtest/player")!);q.httpMethod="POST";q.httpBody=Data("{}".utf8);q.setValue("application/json",forHTTPHeaderField:"Content-Type");q.setValue(AppAuth.defaultOrigin,forHTTPHeaderField:"Origin")
                let (data,response)=try await transport.session.data(for:q)
                guard (response as? HTTPURLResponse)?.statusCode==200 else {return};player=String(decoding:data,as:UTF8.self)
            }
            if AppAuth.shared.loggedIn,let raw=player.data(using:.utf8),let record=(try? JSONSerialization.jsonObject(with:raw)) as? [String:Any],let key=record["key"] as? String {
                var bytes=[UInt8](repeating:0,count:128);let n=bytes.withUnsafeMutableBufferPointer {awb_write(coax_v_u8(data:$0.baseAddress,count:Int64($0.count)))}
                guard n>0 else {throw NativeFailure.refused("Player binding could not be encoded")}
                var q=URLRequest(url:URL(string:AppAuth.defaultOrigin+"/api/chat/world/player/v1")!);q.httpMethod="POST";q.setValue(key,forHTTPHeaderField:"X-Verse-Player");q.setValue(AppAuth.defaultOrigin,forHTTPHeaderField:"Origin");q.setValue("application/json",forHTTPHeaderField:"Content-Type");q.httpBody=Data(bytes.prefix(Int(n)));q=try await AppAuth.shared.authorize(q)
                let (data,response)=try await transport.session.data(for:q);if (response as? HTTPURLResponse)?.statusCode==200 {player=String(decoding:data,as:UTF8.self)}
            }
            guard !ended,!Task.isCancelled else {return}
            try FileManager.default.createDirectory(at:file.deletingLastPathComponent(),withIntermediateDirectories:true,attributes:[.posixPermissions:0o700]);try Data(player.utf8).write(to:file,options:[.atomic,.completeFileProtectionUntilFirstUserAuthentication]);try FileManager.default.setAttributes([.posixPermissions:0o600],ofItemAtPath:file.path)
        }catch {} // Cached public play remains available while offline.
    }
    @discardableResult func perform(_ request:URLRequest,limit:Int=1048576,done:@escaping(Data?,URLResponse?,Error?)->Void)->UUID? {
        guard !ended else {done(nil,nil,CancellationError());return nil}
        let id=UUID()
        jobs[id]=Task { [weak self] in
            guard let self else {return};defer {jobs.removeValue(forKey:id)}
            do {
                var request=request
                let origin=URL(string:AppAuth.defaultOrigin)!
                guard request.url?.host==origin.host,request.url?.scheme==origin.scheme,request.url?.port==origin.port else {throw NativeFailure.refused("World request origin refused")}
                let path=(request.url?.path ?? "")+(request.url?.query.map {"?"+$0} ?? "")
                let allowed=path.withCString {nwp_account(coax_string(data:$0,count:Int64(path.utf8.count)))}
                if allowed,let raw=player.data(using:.utf8),let record=(try? JSONSerialization.jsonObject(with:raw)) as? [String:Any],let key=record["key"] as? String {request.setValue(key,forHTTPHeaderField:"X-Verse-Player")}
                if allowed,AppAuth.shared.loggedIn,request.value(forHTTPHeaderField:"Authorization")==nil {request=try await AppAuth.shared.authorize(request)}
                let (stream,response)=try await transport.session.bytes(for:request);var data=Data()
                for try await byte in stream {guard data.count<limit else {throw NativeFailure.refused("World response exceeded its limit")};data.append(byte)}
                try Task.checkCancellation();if ended {done(nil,nil,CancellationError())}else {done(data,response,nil)}
            }catch {done(nil,nil,error)}
        }
        return id
    }
    func cancel(_ id:UUID) {jobs.removeValue(forKey:id)?.cancel()}
    func viewer()async->String {
        guard AppAuth.shared.loggedIn else {return ""}
        return await withCheckedContinuation {continuation in
            perform(URLRequest(url:URL(string:AppAuth.defaultOrigin+"/api/auth/viewer")!)) {data,response,_ in
                continuation.resume(returning:(response as? HTTPURLResponse)?.statusCode==200 ? String(decoding:data ?? Data(),as:UTF8.self):"")
            }
        }
    }
    func close() {ended=true;for t in jobs.values {t.cancel()};jobs=[:];transport.close()}
}

@MainActor final class NativeWorldHost:UIViewController,MTKViewDelegate,UIAdaptivePresentationControllerDelegate {
    static let deep=Deep()
    private static let assets=NativeWorldAssets()
    let directory:URL,realm:String,path:String,editRequested:Bool
    let finished:()->Void
    private var loading:Task<Void,Never>?,optional:Task<Void,Never>?
    private var mediaTasks:[String:Task<Void,Never>]=[:]
    private var audio:AEPageAudio?
    private let turn=PhoneTurn()
    private let network=NativeWorldNetwork()
    private var page:AEPageHost?
    private var canvas:NativeWorldCanvas!
    private var queue:MTLCommandQueue!
    private var last=0.0
    private var closed=false,editorOffered=false,firstPresented=false
    private var revision:Int32 = -1
    private var editor:NativeWorldEditor?
    private let status=UILabel(),progress=UIProgressView(progressViewStyle:.default)
    private var presentation:MTLCommandBuffer?
    init(directory:URL,realm:String,path:String,edit:Bool,finished:@escaping()->Void) {
        self.directory=directory;self.realm=realm;self.path=path;editRequested=edit;self.finished=finished;super.init(nibName:nil,bundle:nil)
    }
    required init?(coder:NSCoder) {fatalError()}
    static func text(_ s:coax_string)->String {s.count>0 ? String(decoding:UnsafeBufferPointer(start:UnsafeRawPointer(s.data!).assumingMemoryBound(to:UInt8.self),count:Int(s.count)),as:UTF8.self):""}
    override func viewDidLoad() {
        super.viewDidLoad();view.backgroundColor = .black
        let profile=realm.withCString {nwr_profile(coax_string(data:$0,count:Int64(realm.utf8.count)))}
        title=Self.text(profile.title)
        navigationItem.leftBarButtonItem=UIBarButtonItem(systemItem:.close,primaryAction:UIAction { [weak self] _ in self?.leave() })
        navigationItem.leftBarButtonItem?.accessibilityIdentifier="world.close"
        navigationItem.rightBarButtonItem=UIBarButtonItem(image:UIImage(systemName:"slider.horizontal.3"),style:.plain,target:self,action:#selector(openEditor));navigationItem.rightBarButtonItem?.isEnabled=false
        status.numberOfLines=0;status.textColor = .white;status.textAlignment = .center;status.text="Preparing world…";status.translatesAutoresizingMaskIntoConstraints=false
        progress.translatesAutoresizingMaskIntoConstraints=false;view.addSubview(status);view.addSubview(progress)
        NSLayoutConstraint.activate([status.centerYAnchor.constraint(equalTo:view.centerYAnchor),status.leadingAnchor.constraint(equalTo:view.leadingAnchor,constant:24),status.trailingAnchor.constraint(equalTo:view.trailingAnchor,constant:-24),progress.topAnchor.constraint(equalTo:status.bottomAnchor,constant:20),progress.leadingAnchor.constraint(equalTo:status.leadingAnchor),progress.trailingAnchor.constraint(equalTo:status.trailingAnchor)])
        NotificationCenter.default.addObserver(self,selector:#selector(background),name:UIApplication.willResignActiveNotification,object:nil)
        NotificationCenter.default.addObserver(self,selector:#selector(foreground),name:UIApplication.didBecomeActiveNotification,object:nil)
        loading=Task { [weak self] in await self?.open(profile) }
    }
    private func open(_ profile:coax_t_NwProfile)async {
        do {
            let raw=try Data(contentsOf:directory.appendingPathComponent("worlds/\(realm).json"))
            let inventory=try NativeWorldInventory(raw)
            let versions=try PropertyListSerialization.propertyList(from:Data(contentsOf:Bundle.main.url(forResource:"WorldRuntimes",withExtension:"plist")!),format:nil) as! [String:String]
            guard inventory.realm==realm,versions[realm]==inventory.runtime else {throw NativeFailure.refused("This world needs a newer TestFlight build.")}
            status.text="Downloading world assets…"
            let root=try await Self.assets.prepare(inventory) { [weak self] n,total in Task { @MainActor in self?.progress.progress=total>0 ? Float(n)/Float(total):1 } }
            try Task.checkCancellation()
            guard let device=MTLCreateSystemDefaultDevice(),let queue=device.makeCommandQueue() else {throw NativeFailure.refused("Metal is unavailable on this device.")}
            self.queue=queue
            await network.ready();let viewer=await network.viewer();try Task.checkCancellation()
            let c=AEPageConfig();let framework=Self.text(profile.framework)
            c.producer=Bundle.main.privateFrameworksURL!.appendingPathComponent("\(framework).framework/\(framework)").path
            c.shaders=Bundle.main.resourceURL!.appendingPathComponent("WorldShaders/\(realm)").path
            c.pack=Self.text(profile.pack).isEmpty ? "":root.appendingPathComponent("base.pack").path
            c.fonts=root.appendingPathComponent("fonts").path;c.reading="reading";c.title="title";c.titleWeight=realm=="splice-bots" ? 400:600
            c.web=root.appendingPathComponent("web").path;c.store=URL(fileURLWithPath:NSHomeDirectory()).appendingPathComponent(".openagents/world-state/\(path.split(separator:"/").last ?? "world")").path
            c.origin=AppAuth.defaultOrigin;c.path=path+(editRequested ? "?edit":"");c.realm=realm;c.slot=realm;c.style=Self.text(profile.style);c.clock=Self.text(profile.clock);c.variant=Self.text(profile.variant)
            c.touch=true;c.screen=view.bounds.size;c.automated=UserDefaults.standard.bool(forKey:"OAAutomation");c.viewer=viewer
            c.player=network.player.isEmpty ? nil:network.player
            c.layerSide={side in Int32(tf_side(Int64(side),true,tf_pick(0,0,true,0)))}
            c.request={ [weak self] request,done in
                let cancellation=NativeWorldCancellation()
                Task { @MainActor in
                    guard let self,let request,let done else {done?(nil,nil,CancellationError());return}
                    if let id=self.network.perform(request,done:done) {cancellation.bind { [weak self] in Task { @MainActor in self?.network.cancel(id) }}}
                }
                return {cancellation.cancel()}
            }
            c.action={ [weak self] code,text in Task { @MainActor in self?.action(code,text ?? "") } }
            c.assetNeeded={ [weak self] name in
                guard let name else {return};Task { @MainActor in
                    guard let self,!self.closed,self.mediaTasks[name]==nil else {return}
                    self.mediaTasks[name]=Task { [weak self] in
                        defer {self?.mediaTasks.removeValue(forKey:name)}
                        do {try await Self.assets.fetch(name,inventory:inventory);guard let self,!closed,let page else {return};Self.deep.run {page.assetsChanged()}}catch {}
                    }
                }
            }
            status.text="Opening world…";progress.isHidden=true
            let opened:AEPageHost=try await withCheckedThrowingContinuation {continuation in
                Self.deep.begin {var why:NSString?;if let p=AEPageHost.open(c,device:device,queue:queue,why:&why) {continuation.resume(returning:p)}else {continuation.resume(throwing:NativeFailure.refused(why as String? ?? "World could not open"))}}
            }
            // Consume the deep worker's completion before its next call.
            Self.deep.run {}
            if closed || Task.isCancelled {Self.deep.run {opened.close()};return}
            page=opened;status.isHidden=true
            audio=AEPageAudio(page:opened,resources:root,offline:c.automated)
            canvas=NativeWorldCanvas(frame:view.bounds,device:device);canvas.world=self;turn.view=canvas;turn.moved={AppAnalytics.shared.gyro("world")};canvas.delegate=self;canvas.isMultipleTouchEnabled=true;canvas.preferredFramesPerSecond=60;canvas.colorPixelFormat = .bgra8Unorm;canvas.framebufferOnly=false;canvas.translatesAutoresizingMaskIntoConstraints=false
            canvas.accessibilityIdentifier="world.canvas";canvas.isAccessibilityElement=true;canvas.accessibilityTraits = .image;canvas.accessibilityLabel=Self.text(profile.title)
            view.addSubview(canvas);NSLayoutConstraint.activate([canvas.topAnchor.constraint(equalTo:view.safeAreaLayoutGuide.topAnchor),canvas.bottomAnchor.constraint(equalTo:view.safeAreaLayoutGuide.bottomAnchor),canvas.leadingAnchor.constraint(equalTo:view.safeAreaLayoutGuide.leadingAnchor),canvas.trailingAnchor.constraint(equalTo:view.safeAreaLayoutGuide.trailingAnchor)])
            optional=Task { [weak self] in await Self.assets.optional(inventory) {name in Task { @MainActor in
                guard let self,let page=self.page,!self.closed else {return}
                if name=="dragon.frames",let bytes=try? Data(contentsOf:root.appendingPathComponent(name)) {Self.deep.run {_ = page.deliverAsset(4,bytes:bytes)}}
                Self.deep.run {page.assetsChanged()}
            }}}
        }catch {if !closed && !Task.isCancelled {status.text=error.localizedDescription;progress.isHidden=true;navigationItem.rightBarButtonItem=UIBarButtonItem(systemItem:.refresh,primaryAction:UIAction { [weak self] _ in guard let self else {return};loading=Task {await self.open(profile)} })}}
    }
    func draw(in view:MTKView) {
        guard !closed,let page,let drawable=view.currentDrawable,let pass=view.currentRenderPassDescriptor else {return}
        let now=CACurrentMediaTime(),dt=last==0 ? 0:Float(now-last);last=now
        var model:Data?,owns=false;var current:Int32 = -1
        Self.deep.run {_ = page.frame(view.bounds.size,scale:view.contentScaleFactor,dt:dt,wait:false);current=page.editorRevision();if current != revision {model=page.editorModel()};owns=page.editorOwned()}
        audio?.step(dt)
        var wantsText=false,wantsMotion=false;Self.deep.run {wantsText=page.wantsText();wantsMotion=page.wantsMotion()}
        if wantsText && !canvas.isFirstResponder {canvas.becomeFirstResponder()}else if !wantsText && canvas.isFirstResponder {canvas.resignFirstResponder()}
        if wantsMotion {if turn.available {turn.start();let moved=turn.take();Self.deep.run {page.motionStatus(1);page.tilt(Float(moved.yaw),pitch:Float(moved.pitch))}}else {Self.deep.run {page.motionStatus(3)}}}else {turn.stop()}
        if let cmd=queue.makeCommandBuffer(),let encoder=cmd.makeRenderCommandEncoder(descriptor:pass) {
            page.present(encoder,format:view.colorPixelFormat);encoder.endEncoding();cmd.present(drawable)
            if !firstPresented {firstPresented=true;cmd.addCompletedHandler { [weak self] _ in Task { @MainActor in guard let self,!self.closed else {return};self.canvas.accessibilityValue="Ready" }}}
            cmd.commit();presentation=cmd
        }
        if let model {revision=current;editor?.model(String(decoding:model,as:UTF8.self))}
        navigationItem.rightBarButtonItem?.isEnabled=owns || model != nil || editor != nil
        if editRequested,!editorOffered,editor==nil,navigationItem.rightBarButtonItem?.isEnabled==true {editorOffered=true;openEditor()}
    }
    func mtkView(_ view:MTKView,drawableSizeWillChange size:CGSize) {last=0}
    func touch(_ point:CGPoint,phase:Int32,id:Int32) {guard let page else {return};if phase==0 {view.window?.endEditing(true);audio?.gesture()};Self.deep.run {page.touch(point,phase:phase,finger:id)}}
    func text(_ value:String) {guard let page else {return};Self.deep.run {if value=="\n" {_ = page.key(-2,mods:0,down:true);_ = page.key(-2,mods:0,down:false)}else {page.text(value)}}}
    func backspace() {guard let page else {return};Self.deep.run {_ = page.key(-3,mods:0,down:true);_ = page.key(-3,mods:0,down:false)}}
    func send(_ kind:Int32,_ text:String) {
        guard let page else {return}
        if kind==526 {
            let parts=text.split(separator:" ",maxSplits:1).map(String.init)
            guard realm=="splice-bots",parts.count==2,parts[0].hasPrefix("/api/worlds/"),parts[1].withCString({nw_hash_valid(coax_string(data:$0,count:Int64(parts[1].utf8.count)))}) else {return}
            let q=URLRequest(url:URL(string:AppAuth.defaultOrigin+parts[0])!)
            network.perform(q,limit:67108864) { [weak self] data,response,error in
                guard let self,!closed,error==nil,(response as? HTTPURLResponse)?.statusCode==200,let data,NativePackage.hex(Data(SHA256.hash(data:data)))==parts[1],let page=self.page else {return}
                Self.deep.run {_ = page.deliverAsset(0,bytes:data)}
            };return
        }
        Self.deep.run {page.message(kind,text:text)}
    }
    @objc private func openEditor() {
        guard let page,editor==nil else {return}
        do {
            let e=try NativeWorldEditor(directory:directory,network:network,send:{ [weak self] kind,text in self?.send(kind,text) })
            editor=e;var raw:Data?;Self.deep.run {raw=page.editorModel()};if let raw {e.model(String(decoding:raw,as:UTF8.self))}
            e.closed={ [weak self] in self?.editor=nil }
            e.modalPresentationStyle = .pageSheet;e.sheetPresentationController?.detents=[.medium(),.large()];e.sheetPresentationController?.largestUndimmedDetentIdentifier = .medium;e.sheetPresentationController?.prefersScrollingExpandsWhenScrolledToEdge=false;e.sheetPresentationController?.prefersGrabberVisible=true
            present(e,animated:true)
        }catch {status.isHidden=false;status.text=error.localizedDescription}
    }
    private func action(_ code:Int32,_ text:String) {
        if code==2 {UIPasteboard.general.string=text}
        // Navigation is returned to Coax admission; no arbitrary URL is opened.
        if code==1 {
            let target=text.withCString {NativeWorldHost.text(nwp_route(coax_string(data:$0,count:Int64(text.utf8.count))))}
            if target=="catalog" {leave()}
            else if !target.isEmpty {let next=NativeWorldHost(directory:directory,realm:target,path:text,edit:false,finished:finished);close();navigationController?.setViewControllers([next],animated:true)}
        }
        if code==34 {editor?.result(text)}
    }
    @objc private func background() {canvas?.isPaused=true;last=0;turn.stop();audio?.show(false);if let page {Self.deep.run {page.shown(false)}}}
    @objc private func foreground() {last=0;if let page {Self.deep.run {page.shown(true);page.assetsChanged()}};audio?.show(true);canvas?.isPaused=false}
    @objc private func leave() {close();dismiss(animated:true,completion:finished)}
    private func close() {
        guard !closed else {return};closed=true;loading?.cancel();optional?.cancel();for task in mediaTasks.values {task.cancel()};mediaTasks=[:];canvas?.isPaused=true;turn.stop();audio?.audio.close();audio=nil;network.close();presentation?.waitUntilCompleted();if let page {Self.deep.run {page.close()}};page=nil;NotificationCenter.default.removeObserver(self)
    }
    func presentationControllerDidDismiss(_ presentationController:UIPresentationController) {if !closed {close();finished()}}
}

@MainActor final class NativeWorldCanvas:MTKView,UIKeyInput {
    weak var world:NativeWorldHost?
    private var fingers:[ObjectIdentifier:Int32]=[:],nextFinger:Int32=0
    override var canBecomeFirstResponder:Bool {true}
    var hasText:Bool {true}
    func insertText(_ text:String) {world?.text(text)}
    func deleteBackward() {world?.backspace()}
    private func dispatchTouches(_ touches:Set<UITouch>,_ phase:Int32) {
        for touch in touches {let key=ObjectIdentifier(touch);if phase==0 {guard fingers.count<10,nextFinger<Int32.max else {continue};nextFinger+=1;fingers[key]=nextFinger};guard let id=fingers[key] else {continue};world?.touch(touch.location(in:self),phase:phase,id:id);if phase>=2 {fingers.removeValue(forKey:key)}}
    }
    override func touchesBegan(_ touches:Set<UITouch>,with event:UIEvent?) {dispatchTouches(touches,0)}
    override func touchesMoved(_ touches:Set<UITouch>,with event:UIEvent?) {dispatchTouches(touches,1)}
    override func touchesEnded(_ touches:Set<UITouch>,with event:UIEvent?) {dispatchTouches(touches,2)}
    override func touchesCancelled(_ touches:Set<UITouch>,with event:UIEvent?) {dispatchTouches(touches,3)}
}
