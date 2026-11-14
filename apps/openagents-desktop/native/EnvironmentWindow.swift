// Firm reason: 6 (AppKit/JavaScriptCore native projection and account SDK effects).
// The exact Coax application module used by iPhone. No WebView or Markdown
// parser: this projects the existing semantic component tree into AppKit.
import AppKit
import Security

@MainActor final class EnvironmentWindow:NSWindowController,NSTextFieldDelegate,NSTextViewDelegate,NSWindowDelegate {
    static var current:EnvironmentWindow?
    private let program:NativeProgram
    private let stack=NSStackView()
    private var controls:[String:NSView]=[:]
    private var tasks:[UUID:Task<Void,Never>]=[:]
    private var httpTask:Task<Void,Never>?
    private var lifetime=UUID()
    private var httpEpoch=UUID()
    private let auth=MacAccount.shared.auth
    init(directory:URL)throws {
        program=try NativeProgram(directory:directory,module:"app")
        let window=NSWindow(contentRect:NSRect(x:0,y:0,width:720,height:850),styleMask:[.titled,.closable,.miniaturizable,.resizable],backing:.buffered,defer:false)
        super.init(window:window);window.delegate=self;window.title="OpenAgents — Computers";window.minSize=NSSize(width:420,height:500)
        let scroll=NSScrollView();scroll.hasVerticalScroller=true;scroll.drawsBackground=false;scroll.translatesAutoresizingMaskIntoConstraints=false
        let document=NSView();document.translatesAutoresizingMaskIntoConstraints=false
        stack.orientation = .vertical;stack.alignment = .leading;stack.spacing=16;stack.edgeInsets=NSEdgeInsets(top:24,left:24,bottom:24,right:24);stack.translatesAutoresizingMaskIntoConstraints=false
        document.addSubview(stack);scroll.documentView=document;window.contentView!.addSubview(scroll)
        NSLayoutConstraint.activate([scroll.leadingAnchor.constraint(equalTo:window.contentView!.leadingAnchor),scroll.trailingAnchor.constraint(equalTo:window.contentView!.trailingAnchor),scroll.topAnchor.constraint(equalTo:window.contentView!.topAnchor),scroll.bottomAnchor.constraint(equalTo:window.contentView!.bottomAnchor),document.widthAnchor.constraint(equalTo:scroll.contentView.widthAnchor),stack.leadingAnchor.constraint(equalTo:document.leadingAnchor),stack.trailingAnchor.constraint(equalTo:document.trailingAnchor),stack.topAnchor.constraint(equalTo:document.topAnchor),stack.bottomAnchor.constraint(equalTo:document.bottomAnchor)])
        try pump()
    }
    required init?(coder:NSCoder) {fatalError()}
    func windowWillClose(_ notification:Notification) {lifetime=UUID();for task in tasks.values {task.cancel()};tasks=[:];EnvironmentWindow.current=nil}
    private func input(_ id:String,_ value:String?=nil) {
        do {try program.input(id,value);try pump()}catch {showError(error)}
    }
    private func showError(_ error:Error) {let alert=NSAlert();alert.messageText="OpenAgents could not continue";alert.informativeText=error.localizedDescription;if let window {alert.beginSheetModal(for:window)}}
    @objc private func press(_ sender:NSButton) {if let id=sender.identifier?.rawValue {input(id)}}
    func controlTextDidChange(_ notification:Notification) {guard let field=notification.object as? NSTextField,let id=field.identifier?.rawValue else {return};input(id,field.stringValue)}
    func textDidChange(_ notification:Notification) {guard let field=notification.object as? NSTextView,let id=field.identifier?.rawValue else {return};input(id,field.string)}
    private func control(_ element:NativeElement)->NSView {
        if let old=controls[element.id] {update(old,element);return old}
        let view:NSView
        if element.role=="button" {
            let button=NSButton(title:element.label,target:self,action:#selector(press));button.bezelStyle = .rounded;view=button
        } else if element.role=="field" {
            if element.id.hasSuffix("composer") {
                let field=NSTextView();field.delegate=self;field.isRichText=false;field.isAutomaticQuoteSubstitutionEnabled=false;field.font = .systemFont(ofSize:16);field.textContainerInset=NSSize(width:10,height:10);field.heightAnchor.constraint(greaterThanOrEqualToConstant:100).isActive=true;view=field
            } else {let field=NSTextField();field.delegate=self;field.placeholderString=element.label;view=field}
        } else {
            let field=NSTextField(wrappingLabelWithString:element.label);field.isSelectable=true;field.maximumNumberOfLines=0;view=field
        }
        view.identifier=NSUserInterfaceItemIdentifier(element.id);view.setAccessibilityIdentifier(element.id);view.translatesAutoresizingMaskIntoConstraints=false;update(view,element);controls[element.id]=view;return view
    }
    private func update(_ view:NSView,_ element:NativeElement) {
        if let field=view as? NSTextView {if field.string != element.value {field.string=element.value};field.isEditable = !element.disabled;field.setAccessibilityLabel(element.label);return}
        if let button=view as? NSButton {button.title=element.label;button.isEnabled = !element.disabled}
        else if let field=view as? NSTextField {
            let value=element.role=="field" ? element.value:element.label
            if field.stringValue != value {field.stringValue=value}
            field.isEnabled = !element.disabled
            field.font = .systemFont(ofSize:element.role=="heading" ? 28:15)
            field.setAccessibilityLabel(element.label)
        }
    }
    private func render(_ elements:[NativeElement]) {
        var projected:[NativeElement]=[]
        func add(_ e:NativeElement) {
            if e.role=="composer" || e.role=="toolbar" {for child in e.children {add(child)};return}
            if e.role=="message" {
                var message=e
                func text(_ node:NativeElement)->String {node.label+node.children.filter {$0.role != "button"}.map {text($0)}.joined(separator:["p","pre","li","tr"].contains(node.role) ? "\n":"")}
                message.role="text";message.label=text(e);projected.append(message);return
            }
            if e.role != "transcript" {projected.append(e)}
        }
        for e in elements {add(e)}
        let ids=Set(projected.map(\.id))
        for (id,view) in controls where !ids.contains(id) {stack.removeArrangedSubview(view);view.removeFromSuperview();controls.removeValue(forKey:id)}
        for (index,e) in projected.enumerated() {
            let view=control(e)
            if !stack.arrangedSubviews.contains(view) {stack.insertArrangedSubview(view,at:min(index,stack.arrangedSubviews.count));view.widthAnchor.constraint(equalTo:stack.widthAnchor,constant:-48).isActive=true}
        }
    }
    private func pump()throws {
        try program.frame();render(program.elements())
        if let slot=program.focus,let id=program.nodes[slot]?.id {window?.makeFirstResponder(controls[id]);program.focus=nil}
        let (op,body)=try program.action();guard op != 0 else {return}
        let args=(try? JSONSerialization.jsonObject(with:Data(body.utf8))) as? [String:Any] ?? [:]
        if op==8 {httpEpoch=UUID();httpTask?.cancel();httpTask=nil};let owner=httpEpoch,viewOwner=lifetime,job=UUID()
        let task=Task { [weak self] in
            guard let self else {return}
            defer {tasks.removeValue(forKey:job)}
            do {
                let result=try await call(op,args)
                guard viewOwner==lifetime,op != 4 || owner==httpEpoch else {return}
                let raw=(op==4 || op==14) ? result["body"] as? String ?? "":String(decoding:try JSONSerialization.data(withJSONObject:result),as:UTF8.self)
                try program.event(100+op,result["status"] as? Int ?? 0,raw);try pump()
            }catch {
                guard viewOwner==lifetime,op != 4 || owner==httpEpoch else {return}
                let raw=String(decoding:(try? JSONSerialization.data(withJSONObject:["error":error.localizedDescription])) ?? Data(),as:UTF8.self)
                do {try program.event(200+op,0,raw);try pump()}catch {showError(error)}
            }
        };tasks[job]=task;if op==4 {httpTask=task}
    }
    private func call(_ op:Int,_ args:[String:Any])async throws->[String:Any] {
        switch op {
        case 1:return ["loggedIn":auth.loggedIn,"localEnvironment":FileManager.default.fileExists(atPath:MacAccount.home.appendingPathComponent(".openagents/coder/host/service.sock").path)]
        case 2:return ["pending":try await auth.emailStart(args["email"] as? String ?? "")]
        case 3:_ = try await auth.emailVerify(code:args["code"] as? String ?? "",pending:args["pending"] as? String ?? "");return ["loggedIn":true]
        case 5:await auth.logout();return ["loggedIn":false]
        case 6,7:return ["message":"OpenAgents desktop — Coax Native environment client"]
        case 8:return ["cancelled":true]
        case 9:NSPasteboard.general.clearContents();NSPasteboard.general.setString(args["text"] as? String ?? "",forType:.string);return ["copied":true]
        case 10:
            guard let raw=args["url"] as? String,raw.withCString({nt_link(coax_string(data:$0,count:Int64(raw.utf8.count)))}),let url=URL(string:raw) else {throw NativeFailure.refused("Link refused")}
            NSWorkspace.shared.open(url);return ["opened":true]
        case 11:
            var bytes=[UInt8](repeating:0,count:32);guard SecRandomCopyBytes(kSecRandomDefault,bytes.count,&bytes)==errSecSuccess else {throw NativeFailure.refused("Random identities unavailable")}
            return ["command_id":bytes.prefix(16).map {String(format:"%02x",$0)}.joined(),"session_id":bytes.suffix(16).map {String(format:"%02x",$0)}.joined(),"now_ms":Int64(Date().timeIntervalSince1970*1000)]
        case 12:try await Task.sleep(for:.seconds(1));return ["ready":true]
        case 14:
            let raw=String(decoding:try JSONSerialization.data(withJSONObject:args),as:UTF8.self)
            let result=try await Self.localCall(raw)
            return ["status":200,"body":result]
        case 4:
            let path=args["path"] as? String ?? "",method=args["method"] as? String ?? "GET",body=args["body"] as? String ?? ""
            guard path.withCString({cn_api(coax_string(data:$0,count:Int64(path.utf8.count)))}),["GET","POST"].contains(method),body.utf8.count<=65536 else {throw NativeFailure.refused("Request refused")}
            var request=URLRequest(url:URL(string:MacAccount.origin+path)!,timeoutInterval:60);request.httpMethod=method
            if method=="POST" {request.httpBody=Data(body.utf8);request.setValue("application/json",forHTTPHeaderField:"Content-Type")}
            if auth.loggedIn {request=try await auth.authorize(request)}
            let (stream,response)=try await URLSession.shared.bytes(for:request);var data=Data()
            guard response.url?.host==URL(string:MacAccount.origin)?.host else {throw NativeFailure.refused("Redirect refused")}
            for try await byte in stream {guard data.count<1048576 else {throw NativeFailure.refused("Response too large")};data.append(byte)}
            return ["status":(response as? HTTPURLResponse)?.statusCode ?? 500,"body":String(decoding:data,as:UTF8.self)]
        default:throw NativeFailure.refused("Service unavailable")
        }
    }
    private static func localCall(_ raw:String)async throws->String {
        try await Task.detached {
            let folder=MacAccount.folder.appendingPathComponent("requests",isDirectory:true)
            try FileManager.default.createDirectory(at:folder,withIntermediateDirectories:true,attributes:[.posixPermissions:0o700])
            let file=folder.appendingPathComponent(UUID().uuidString+".json"),data=Data(raw.utf8)
            let fd=open(file.path,O_WRONLY|O_CREAT|O_EXCL,0o600);guard fd>=0 else {throw NativeFailure.refused("Cannot prepare local request")}
            let wrote=data.withUnsafeBytes {Darwin.write(fd,$0.baseAddress,data.count)};Darwin.close(fd)
            defer {try? FileManager.default.removeItem(at:file)}
            guard wrote==data.count else {throw NativeFailure.refused("Cannot keep local request")}
            let process=Process(),pipe=Pipe();process.executableURL=MacAccount.home.appendingPathComponent(".openagents/bin/openagents-environment");process.arguments=["local-rpc","--request-file",file.path];process.standardOutput=pipe;process.standardError=FileHandle.nullDevice
            try process.run();let output=pipe.fileHandleForReading.readDataToEndOfFile();process.waitUntilExit()
            guard process.terminationStatus==0,output.count<=65536 else {throw NativeFailure.refused("This computer's environment service is unavailable")}
            return String(decoding:output,as:UTF8.self)
        }.value
    }
}

@_cdecl("oa_environments_open") func oa_environments_open() {
    MainActor.assumeIsolated {
        do {if EnvironmentWindow.current==nil {guard let directory=Bundle.main.url(forResource:"Starter",withExtension:nil) else {throw NativeFailure.refused("Native application resources unavailable")};EnvironmentWindow.current=try EnvironmentWindow(directory:directory)};EnvironmentWindow.current?.showWindow(nil)}
        catch {let alert=NSAlert();alert.messageText=error.localizedDescription;alert.runModal()}
    }
}
