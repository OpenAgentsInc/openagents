// Firm reason: 6 (UIKit navigation and network effects for the Coax world library).
import UIKit
import UniformTypeIdentifiers

@MainActor final class NativeWorldLibrary:UIViewController,UIDocumentPickerDelegate {
    private let program:NativeProgram,controls=NativeControls(),network=NativeWorldNetwork()
    private let service:(String,[String:Any])async throws->Any
    private let finished:()->Void
    private var job:Task<Void,Never>?
    private var documentTarget="",documentFields:[String:Any]=[:]
    init(directory:URL,service:@escaping(String,[String:Any])async throws->Any,finished:@escaping()->Void)throws {
        program=try NativeProgram(directory:directory,module:"worlds");self.service=service;self.finished=finished;super.init(nibName:nil,bundle:nil)
    }
    required init?(coder:NSCoder) {fatalError()}
    override func viewDidLoad() {
        super.viewDidLoad();title="Worlds";navigationItem.leftBarButtonItem=UIBarButtonItem(systemItem:.close,primaryAction:UIAction { [weak self] _ in self?.leave() })
        addChild(controls);controls.view.frame=view.bounds;controls.view.autoresizingMask=[.flexibleWidth,.flexibleHeight];view.addSubview(controls.view);controls.didMove(toParent:self)
        controls.press={ [weak self] id in self?.input(id) };controls.edit={ [weak self] id,text in self?.input(id,text) }
        job=Task { [weak self] in guard let self else {return};await network.ready();guard !Task.isCancelled else {return};pump() }
    }
    private func input(_ id:String,_ text:String?=nil) {do {try program.input(id,text);pump()}catch {failure(error)}}
    private func failure(_ error:Error) {controls.render([NativeElement(id:"worlds.failure",role:"notice",label:error.localizedDescription)])}
    private func pump() {
        do {
            try program.frame();controls.render(program.elements());let (op,body)=try program.action();guard op != 0 else {return}
            if op==31 {
                let parts=body.split(separator:"\n",maxSplits:1).map(String.init);documentTarget=parts.first ?? "";documentFields=parts.count==2 ? ((try? JSONSerialization.jsonObject(with:Data(parts[1].utf8))) as? [String:Any] ?? [:]):[:]
                let picker=UIDocumentPickerViewController(forOpeningContentTypes:[.plainText,.pdf,UTType(filenameExtension:"docx") ?? .data,UTType(filenameExtension:"md") ?? .plainText],asCopy:true);picker.delegate=self;present(picker,animated:true);return
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
    func documentPicker(_ controller:UIDocumentPickerViewController,didPickDocumentsAt urls:[URL]) {
        guard let url=urls.first,documentTarget.withCString({nwp_account(coax_string(data:$0,count:Int64(documentTarget.utf8.count)))}) else {return}
        do {
            let size=try url.resourceValues(forKeys:[.fileSizeKey]).fileSize ?? 0
            if size>360*1024 {try program.event(54,0,"413\ntoo large");pump();return}
            try program.event(54,0,"1\n\(url.lastPathComponent)");pump()
            var fields=documentFields;fields["name"]=url.lastPathComponent;fields["data"]=try Data(contentsOf:url).base64EncodedString()
            var q=URLRequest(url:URL(string:AppAuth.defaultOrigin+documentTarget)!);q.httpMethod="POST";q.httpBody=try JSONSerialization.data(withJSONObject:fields);q.setValue("application/json",forHTTPHeaderField:"Content-Type");q.setValue(AppAuth.defaultOrigin,forHTTPHeaderField:"Origin")
            network.perform(q) { [weak self] data,response,error in guard let self else {return};do {try program.event(54,0,"\(error==nil ? (response as? HTTPURLResponse)?.statusCode ?? 0:0)\n"+String(decoding:data ?? Data(),as:UTF8.self));pump()}catch {failure(error)}}
        }catch {failure(error)}
    }
    func documentPickerWasCancelled(_ controller:UIDocumentPickerViewController) {do {try program.event(55,0,"");pump()}catch {failure(error)}}
    private func leave() {job?.cancel();network.close();dismiss(animated:true,completion:finished)}
}
