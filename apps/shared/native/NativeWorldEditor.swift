// Firm reason: 6 (UIKit sheet and native projection of the existing Coax editor).
import UIKit
import UniformTypeIdentifiers
import CryptoKit

@MainActor final class NativeWorldEditor:UIViewController,UIAdaptivePresentationControllerDelegate,UIDocumentPickerDelegate {
    private let program:NativeProgram
    private let controls=NativeControls()
    private let network:NativeWorldNetwork
    private let send:(Int32,String)->Void
    private let directory:URL
    private var ended=false,wantsChat=false
    private var pickerID:String?,offered:Data?
    private var conversation:NativeWorldConversation?
    private let upload=NativeWorldSession()
    private var uploadJob:Task<Void,Never>?
    var closed:(()->Void)?
    init(directory:URL,network:NativeWorldNetwork,send:@escaping(Int32,String)->Void)throws {
        self.directory=directory;program=try NativeProgram(directory:directory,module:"editor");self.network=network;self.send=send;super.init(nibName:nil,bundle:nil)
        try program.event(16,0,"") // native navigation and agent sheet
        if AppAuth.shared.loggedIn {try program.event(12,0,"")}
    }
    required init?(coder:NSCoder) {fatalError()}
    override func viewDidLoad() {
        super.viewDidLoad();addChild(controls);controls.view.frame=view.bounds;controls.view.autoresizingMask=[.flexibleWidth,.flexibleHeight];view.addSubview(controls.view);controls.didMove(toParent:self)
        controls.press={ [weak self] id in self?.input(id) };controls.edit={ [weak self] id,text in self?.input(id,text) };controls.submit={ [weak self] id in self?.submit(id) }
        presentationController?.delegate=self;pump()
    }
    func model(_ raw:String) {do {try program.event(6,0,raw);pump()}catch {failure(error)}}
    func result(_ raw:String) {conversation?.result(raw)}
    private func input(_ id:String,_ text:String?=nil) {
        if text==nil,let node=program.nodes.values.first(where:{$0.id==id}),let accept=node.attrs["data-drop"] {pick(id,accept:accept);return}
        if id=="agent" {wantsChat=true}
        do {try program.input(id,text);pump()}catch {failure(error)}
    }
    private func submit(_ id:String) {guard let slot=program.nodes.first(where:{$0.value.id==id})?.key else {return};do {try program.event(3,slot,"");pump()}catch {failure(error)}}
    private func failure(_ error:Error) {controls.render([NativeElement(id:"editor.failure",role:"notice",label:error.localizedDescription)])}
    private func pump() {
        guard !ended else {return}
        do {
            try program.frame();controls.render(program.elements())
            for _ in 0..<8 {
                let (kind,body)=try program.action();if kind==0 {break}
                if [520,521,522,523,525,526,527,530].contains(kind) {send(Int32(kind),body)}
                else if [401,402,403].contains(kind) {request(kind,body)}
                else if kind==404 {if let d=body.data(using:.utf8),let b=(try? JSONSerialization.jsonObject(with:d)) as? [String:String] {UIPasteboard.general.string=b["text"]}}
                else if kind==529 {finish();dismiss(animated:true)}
                else if kind==528 {try program.event(13,0,"")}
                else if kind==524 {
                    if let conversation {conversation.context(body)}
                    else {conversation=try NativeWorldConversation(directory:directory,network:network,context:body) { [weak self] plan in
                        guard let self,!ended else {return};do {try program.event(8,0,plan);pump()}catch {failure(error)}
                    }}
                    if wantsChat,let conversation,presentedViewController==nil {wantsChat=false;conversation.resume();conversation.modalPresentationStyle = .pageSheet;present(conversation,animated:true)}
                }
            }
        }catch {failure(error)}
    }
    private func request(_ kind:Int,_ body:String) {
        guard let raw=body.data(using:.utf8),let args=try? JSONSerialization.jsonObject(with:raw) as? [String:Any],let path=args["url"] as? String else {return}
        let text=args["body"] as? String ?? ""
        let step=args["step"] as? Int ?? 0
        if kind==403 && step==2 {
            guard let offered,let url=URL(string:path),url.scheme=="https",url.host != nil,url.user==nil,url.password==nil else {reply(kind,400,"");return}
            uploadJob=Task { [weak self] in
                guard let self else {return};do {
                    var q=URLRequest(url:url);q.httpMethod="PUT" // no cookies, bearer, player key, Origin or content-type
                    let (_,response)=try await upload.session.upload(for:q,from:offered);guard !ended else {return};reply(kind,(response as? HTTPURLResponse)?.statusCode ?? 0,"")
                }catch {if !ended {reply(kind,0,"")}}
            };return
        }
        guard path.withCString({nwp_account(coax_string(data:$0,count:Int64(path.utf8.count)))}) else {reply(kind,403,"This operation is unavailable in the installed app.");return}
        var q=URLRequest(url:URL(string:AppAuth.defaultOrigin+path)!)
        let method=kind==402 && text.isEmpty ? "GET":"POST";q.httpMethod=method;q.setValue(AppAuth.defaultOrigin,forHTTPHeaderField:"Origin")
        if method=="POST" {
            var payload=Data(text.utf8)
            if kind==402,let offered {payload.append(offered);self.offered=nil}
            if kind==403 && step==1 {guard let offered else {reply(kind,400,"");return};payload.append(Data((NativePackage.hex(Data(SHA256.hash(data:offered)))+"\n").utf8))}
            if kind==403 && step==3 {offered=nil}
            q.httpBody=payload;q.setValue("application/octet-stream",forHTTPHeaderField:"Content-Type")
        }
        network.perform(q) { [weak self] data,response,error in
            guard let self,!ended else {return};reply(kind,error==nil ? (response as? HTTPURLResponse)?.statusCode ?? 0:0,String(decoding:data ?? Data(),as:UTF8.self))
        }
    }
    private func pick(_ id:String,accept:String) {
        guard presentedViewController==nil else {return};pickerID=id
        let types=accept.split(separator:",").compactMap {UTType(filenameExtension:String($0).trimmingCharacters(in:CharacterSet(charactersIn:". ")))}
        let picker=UIDocumentPickerViewController(forOpeningContentTypes:types.isEmpty ? [.data]:types,asCopy:true);picker.delegate=self;present(picker,animated:true)
    }
    func documentPicker(_ controller:UIDocumentPickerViewController,didPickDocumentsAt urls:[URL]) {
        guard let id=pickerID,let url=urls.first,let slot=program.nodes.first(where:{$0.value.id==id})?.key else {return};pickerID=nil
        do {
            let size=try url.resourceValues(forKeys:[.fileSizeKey]).fileSize ?? 0
            offered=size<=67108864 ? try Data(contentsOf:url,options:.mappedIfSafe):nil
            try program.event(10,slot,"\(size) \(url.lastPathComponent)");pump()
        }catch {failure(error)}
    }
    func documentPickerWasCancelled(_ controller:UIDocumentPickerViewController) {pickerID=nil}
    private func reply(_ kind:Int,_ status:Int,_ body:String) {
        do {if kind==401 {try program.event(4,status,body);try program.event(5,status,"")}else {try program.event(11,status,body)};pump()}catch {failure(error)}
    }
    private func finish() {guard !ended else {return};ended=true;uploadJob?.cancel();upload.close();offered=nil;closed?()}
    func presentationControllerDidDismiss(_ presentationController:UIPresentationController) {finish()}
}
