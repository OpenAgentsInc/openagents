// Firm reason: 6 (UIKit/WebKit presentation and native account SDK effects).
import UIKit
import WebKit
import AuthenticationServices

@MainActor final class NativeFiles: NSObject, WKURLSchemeHandler {
    let directory: URL
    init(_ directory: URL) { self.directory = directory }
    func webView(_ webView: WKWebView,start task: WKURLSchemeTask) {
        guard let url = task.request.url, url.host == "app" else { task.didFailWithError(NativeFailure.refused("Invalid package origin")); return }
        let name = url.path == "/" ? "index.html" : String(url.path.dropFirst())
        guard name.withCString({cnu_path(coax_string(data:$0,count:Int64(name.utf8.count)))}) else { task.didFailWithError(NativeFailure.refused("Invalid package path")); return }
        do {
            let file = directory.appendingPathComponent(name)
            let data = try Data(contentsOf:file)
            let types = ["html":"text/html","js":"text/javascript","wasm":"application/wasm","css":"text/css","ttf":"font/ttf","svg":"image/svg+xml"]
            let response = HTTPURLResponse(url:url,statusCode:200,httpVersion:"HTTP/1.1",headerFields:["Content-Type":types[file.pathExtension] ?? "application/octet-stream","Access-Control-Allow-Origin":"*","Cache-Control":"no-store"])!
            task.didReceive(response); task.didReceive(data); task.didFinish()
        } catch { task.didFailWithError(error) }
    }
    func webView(_ webView: WKWebView,stop task: WKURLSchemeTask) {}
}

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

@MainActor final class NativeHost: UIViewController, WKScriptMessageHandlerWithReply, WKNavigationDelegate {
    var web: WKWebView!
    let updates: NativeUpdates
    var files: NativeFiles?
    var epoch = UUID()
    var services = 0
    var health: Task<Void,Never>?
    var passkey: NativePasskey?
    init(updates:NativeUpdates) { self.updates=updates; super.init(nibName:nil,bundle:nil) }
    required init?(coder:NSCoder) { fatalError() }
    override func viewDidLoad() { super.viewDidLoad(); view.backgroundColor = .black; load() }
    func load() {
        epoch=UUID(); health?.cancel(); web?.configuration.userContentController.removeScriptMessageHandler(forName:"coaxNative",contentWorld:.page); web?.removeFromSuperview()
        let config=WKWebViewConfiguration(); config.allowsInlineMediaPlayback=true
        files=NativeFiles(updates.boot()); config.setURLSchemeHandler(files,forURLScheme:"coax-native")
        config.userContentController.addScriptMessageHandler(self,contentWorld:.page,name:"coaxNative")
        web=WKWebView(frame:view.bounds,configuration:config); web.autoresizingMask=[.flexibleWidth,.flexibleHeight]; web.navigationDelegate=self; web.isOpaque=false; web.backgroundColor = .black
        web.scrollView.contentInsetAdjustmentBehavior = .never
        #if DEBUG
        web.isInspectable=true
        #endif
        view.addSubview(web); web.load(URLRequest(url:URL(string:"coax-native://app/index.html")!))
    }
    func webView(_ webView:WKWebView,decidePolicyFor action:WKNavigationAction,decisionHandler:@escaping(WKNavigationActionPolicy)->Void) {
        guard let url=action.request.url else { decisionHandler(.cancel); return }
        if url.scheme == "coax-native" && url.host == "app" { decisionHandler(.allow); return }
        decisionHandler(.cancel)
        if ["https","http"].contains(url.scheme ?? "") { UIApplication.shared.open(url) }
    }
    func webViewWebContentProcessDidTerminate(_ webView:WKWebView) { updates.failed(); load() }
    func userContentController(_ controller:WKUserContentController,didReceive message:WKScriptMessage,replyHandler:@escaping(Any?,String?)->Void) {
        guard message.frameInfo.isMainFrame, message.frameInfo.securityOrigin.protocol == "coax-native", message.frameInfo.securityOrigin.host == "app", let b=message.body as? [String:Any], let op=b["op"] as? String else { replyHandler(nil,"Native service origin refused"); return }
        guard services<8 else {replyHandler(nil,"Native services busy");return}
        let owner=epoch
        services += 1
        Task { [weak self] in
            guard let self else { replyHandler(nil,"Surface retired"); return }
            defer { self.services -= 1 }
            do { let result=try await self.call(op,b); guard self.epoch == owner else { replyHandler(nil,"Surface retired"); return }; replyHandler(result,nil) }
            catch { replyHandler(nil,error.localizedDescription) }
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
        case "auth.state": return ["loggedIn":AppAuth.shared.loggedIn]
        case "auth.emailStart": return ["pending":try await AppAuth.shared.emailStart(b["email"] as? String ?? "")]
        case "auth.emailVerify": _ = try await AppAuth.shared.emailVerify(code:b["code"] as? String ?? "",pending:b["pending"] as? String ?? ""); return ["loggedIn":true]
        case "auth.keyLogin": _ = try await AppAuth.shared.keyLogin(); return ["loggedIn":true]
        case "auth.passkeyLogin":
            guard let window=view.window, passkey==nil else { throw NativeFailure.refused("Authentication busy") }
            let challenge=try await AppAuth.shared.passkeyChallenge(); let p=NativePasskey(window); passkey=p; defer { passkey=nil }
            _ = try await AppAuth.shared.passkeyLogin(challenge,try await p.get(challenge)); return ["loggedIn":true]
        case "auth.logout": await AppAuth.shared.logout(); return ["loggedIn":false]
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
