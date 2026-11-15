// Firm reason: 6 (UIKit navigation and network effects for the Coax world library).
import UIKit
import UniformTypeIdentifiers

@MainActor final class NativeWorldLibrary:UIViewController {
    private let program:NativeProgram,network=NativeWorldNetwork()
    let controls=NativeControls()
    private let service:(String,[String:Any])async throws->Any
    private let finished:()->Void
    private var job:Task<Void,Never>?
    private var clock:Timer?
    private var ready=false
    private let navigation:(()->Void)?
    private var documentTarget="",documentFields:[String:Any]=[:]
    private let documents=NativeDocuments()
    init(directory:URL,service:@escaping(String,[String:Any])async throws->Any,navigation:(()->Void)?=nil,finished:@escaping()->Void)throws {
        program=try NativeProgram(directory:directory,module:"worlds");self.service=service;self.navigation=navigation;self.finished=finished;super.init(nibName:nil,bundle:nil)
    }
    required init?(coder:NSCoder) {fatalError()}
    override func viewDidLoad() {
        // The library draws its own header in the skin; the navigation controller only presents it.
        super.viewDidLoad();title="Worlds";navigationController?.setNavigationBarHidden(true,animated:false)
        if navigation==nil {controls.hostHeader(title:"Worlds",close:"Close") { [weak self] in self?.leave() }}
        addChild(controls);controls.view.frame=view.bounds;controls.view.autoresizingMask=[.flexibleWidth,.flexibleHeight];view.addSubview(controls.view);controls.didMove(toParent:self)
        controls.press={ [weak self] id in self?.input(id) };controls.edit={ [weak self] id,text in self?.input(id,text) }
        job=Task { [weak self] in guard let self else {return};await network.ready();guard !Task.isCancelled else {return};ready=true;pump() }
        clock=Timer.scheduledTimer(withTimeInterval:0.5,repeats:true) { [weak self] _ in Task { @MainActor [weak self] in guard let self,self.ready,UIApplication.shared.applicationState == .active,self.view.window != nil,self.presentedViewController==nil else {return};self.pump(elapsed:0.5) }}
    }
    private func input(_ id:String,_ text:String?=nil) {if id=="nav.toggle",let navigation {navigation();return};do {try program.input(id,text);pump()}catch {failure(error)}}
    private func failure(_ error:Error) {controls.render([NativeElement(id:"worlds.failure",role:"notice",label:error.localizedDescription)])}
    private func pump(elapsed:Double=0) {
        do {
            try program.frame(elapsed:elapsed);controls.render(program.elements());let (op,body)=try program.action();guard op != 0 else {return}
            if op==31 {
                let parts=body.split(separator:"\n",maxSplits:1).map(String.init);documentTarget=parts.first ?? "";documentFields=parts.count==2 ? ((try? JSONSerialization.jsonObject(with:Data(parts[1].utf8))) as? [String:Any] ?? [:]):[:]
                job=Task { [weak self] in await self?.pickDocument() };return
            }
            let args=(try JSONSerialization.jsonObject(with:Data(body.utf8))) as? [String:Any] ?? [:]
            if op==4,let path=args["path"] as? String,path.withCString({nwp_account(coax_string(data:$0,count:Int64(path.utf8.count)))}) {
                var q=URLRequest(url:URL(string:AppAuth.defaultOrigin+path)!);q.httpMethod=args["method"] as? String ?? "GET"
                if q.httpMethod=="POST" {q.httpBody=Data((args["body"] as? String ?? "").utf8);q.setValue("application/json",forHTTPHeaderField:"Content-Type");q.setValue(AppAuth.defaultOrigin,forHTTPHeaderField:"Origin")}
                network.perform(q) { [weak self] data,response,error in
                    guard let self else {return};do {try program.event(error==nil ? 104:204,(response as? HTTPURLResponse)?.statusCode ?? 0,String(decoding:data ?? Data(),as:UTF8.self));pump()}catch {failure(error)}
                }
            } else if op==13 {
                job=Task { [weak self] in
                    guard let self else {return}
                    do {_ = try await service("world.open",["nativeWire":body,"worldPresenter":self]);try program.event(113,0,"{\"closed\":true}");pump()}
                    catch {do {try program.event(213,0,"");pump()}catch {failure(error)}}
                }
            } else {throw NativeFailure.refused("Unsupported world operation")}
        }catch {failure(error)}
    }
    private func pickDocument()async {
        guard documentTarget.withCString({nwp_account(coax_string(data:$0,count:Int64(documentTarget.utf8.count)))}) else {return}
        do {
            let chosen=try await documents.pick(self,types:[.plainText,.pdf,UTType(filenameExtension:"docx") ?? .data,UTType(filenameExtension:"md") ?? .plainText],limit:360*1024)
            defer {documents.release()}
            if chosen["cancelled"] as? Bool == true {try program.event(55,0,"");pump();return}
            guard let token=chosen["token"] as? String,let data=documents.data(token),let name=chosen["name"] as? String else {throw NativeFailure.refused("The document is unavailable.")}
            try program.event(54,0,"1\n\(name)");pump()
            var fields=documentFields;fields["name"]=name;fields["data"]=data.base64EncodedString()
            var q=URLRequest(url:URL(string:AppAuth.defaultOrigin+documentTarget)!);q.httpMethod="POST";q.httpBody=try JSONSerialization.data(withJSONObject:fields);q.setValue("application/json",forHTTPHeaderField:"Content-Type");q.setValue(AppAuth.defaultOrigin,forHTTPHeaderField:"Origin")
            network.perform(q) { [weak self] data,response,error in guard let self else {return};do {try program.event(54,0,"\(error==nil ? (response as? HTTPURLResponse)?.statusCode ?? 0:0)\n"+String(decoding:data ?? Data(),as:UTF8.self));pump()}catch {failure(error)}}
        }catch {do {try program.event(54,0,"413\n"+error.localizedDescription);pump()}catch {failure(error)}}
    }
    func stop() {clock?.invalidate();clock=nil;job?.cancel();network.close()}
    private func leave() {stop();dismiss(animated:true,completion:finished)}
    deinit {clock?.invalidate()}
}
