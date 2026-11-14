// Firm reason: 6 (UIKit sheet and installed HTTP/clipboard effects; the account
// chat module, composer, Markdown and typed world plans stay shared Coax).
import UIKit

@MainActor final class NativeWorldConversation:UIViewController {
    private let program:NativeProgram,controls=NativeControls(),network:NativeWorldNetwork
    private let planned:(String)->Void
    private var request:UUID?,epoch=UUID(),ended=false
    init(directory:URL,network:NativeWorldNetwork,context:String,planned:@escaping(String)->Void)throws {
        program=try NativeProgram(directory:directory,module:"app");self.network=network;self.planned=planned;super.init(nibName:nil,bundle:nil);try program.event(16,0,context)
    }
    required init?(coder:NSCoder) {fatalError()}
    override func viewDidLoad() {
        super.viewDidLoad();addChild(controls);controls.view.frame=view.bounds;controls.view.autoresizingMask=[.flexibleWidth,.flexibleHeight];view.addSubview(controls.view);controls.didMove(toParent:self)
        controls.press={ [weak self] id in self?.input(id) };controls.edit={ [weak self] id,text in self?.input(id,text) };pump()
    }
    func context(_ text:String) {do {try program.event(16,0,text);pump()}catch {failure(error)}}
    func resume() {ended=false;epoch=UUID();pump()}
    func result(_ text:String) {do {try program.event(17,0,text);pump()}catch {failure(error)}}
    private func input(_ id:String,_ text:String?=nil) {do {try program.input(id,text);pump()}catch {failure(error)}}
    private func failure(_ error:Error) {controls.render([NativeElement(id:"world.chat.failure",role:"notice",label:error.localizedDescription)])}
    private func pump() {
        guard !ended else {return}
        do {
            try program.frame();controls.render(program.elements());let (op,body)=try program.action();if op==0 {return}
            if op==529 {finish();return}
            if op==520 {planned(body);return}
            let args=(try? JSONSerialization.jsonObject(with:Data(body.utf8))) as? [String:Any] ?? [:]
            if op==4,let path=args["path"] as? String,path.withCString({nwp_account(coax_string(data:$0,count:Int64(path.utf8.count)))}) {
                isModalInPresentation=true
                var q=URLRequest(url:URL(string:AppAuth.defaultOrigin+path)!);q.httpMethod=args["method"] as? String ?? "GET"
                if q.httpMethod=="POST" {q.httpBody=Data((args["body"] as? String ?? "").utf8);q.setValue("application/json",forHTTPHeaderField:"Content-Type");q.setValue(AppAuth.defaultOrigin,forHTTPHeaderField:"Origin")}
                let owner=epoch
                request=network.perform(q) { [weak self] data,response,error in
                    guard let self,!ended,owner==epoch else {return};request=nil;isModalInPresentation=false
                    do {try program.event(error==nil ? 104:204,(response as? HTTPURLResponse)?.statusCode ?? 0,String(decoding:data ?? Data(),as:UTF8.self));pump()}catch {failure(error)}
                }
            } else if op==8 {epoch=UUID();if let request {network.cancel(request)};request=nil;isModalInPresentation=false;try program.event(108,0,"{}");pump()}
            else if op==9 {UIPasteboard.general.string=args["text"] as? String;try program.event(109,0,"{}");pump()}
            else if op==10,let text=args["url"] as? String,text.withCString({nt_link(coax_string(data:$0,count:Int64(text.utf8.count)))}),let url=URL(string:text) {UIApplication.shared.open(url);try program.event(110,0,"{}");pump()}
        }catch {failure(error)}
    }
    private func finish() {ended=true;epoch=UUID();if let request {network.cancel(request)};dismiss(animated:true)}
    override func viewDidDisappear(_ animated:Bool) {super.viewDidDisappear(animated);if isBeingDismissed || navigationController?.isBeingDismissed==true {ended=true;if let request {network.cancel(request)}}}
}
