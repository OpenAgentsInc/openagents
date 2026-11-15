// Firm reason: 6 (Keychain, LocalAuthentication, AuthenticationServices, push and WebSocket SDK effects).
// Coax owns screen state, admission, input codecs and Nostr protocol bytes.
import UIKit
import Security
import LocalAuthentication
import UserNotifications

enum NativeAccountHTTP:Error,LocalizedError {
    case refused(Int,String)
    var errorDescription:String? {if case .refused(_,let words)=self {return words};return nil}
}

enum NativeAccountCodec {
    private static let transport=NativeWorldSession()
    static let network:AppAuth.Send = {try await transport.fetch($0,limit:131072)}
    static func text<T>(_ value:String,_ body:(coax_string)->T)->T {value.withCString {body(coax_string(data:$0,count:Int64(value.utf8.count)))}}
    static func bytes<T>(_ value:Data,_ body:(coax_v_u8)->T)->T {var copy=value;defer {copy.resetBytes(in:0..<copy.count)};return copy.withUnsafeMutableBytes {body(coax_v_u8(data:$0.bindMemory(to:UInt8.self).baseAddress,count:Int64($0.count)))}}
    static func write(_ cap:Int=65536,_ body:(coax_v_u8)->Int64)throws->String {
        var output=[UInt8](repeating:0,count:cap);defer {output.withUnsafeMutableBytes {$0.initializeMemory(as:UInt8.self,repeating:0)}}
        let n=output.withUnsafeMutableBufferPointer {body(coax_v_u8(data:$0.baseAddress,count:Int64($0.count)))}
        guard n>=0,n<=cap else {throw NativeFailure.refused("Account unavailable")};return String(decoding:output.prefix(Int(n)),as:UTF8.self)
    }
    static func part(_ raw:String,_ path:String)->String {(try? write(8192) {out in text(raw) {r in text(path) {nas_part(r,$0,out)}}}) ?? ""}
    static func flag(_ raw:String,_ path:String)->Bool {text(raw) {r in text(path) {nas_flag(r,$0)}}}
    static func encode(_ key:Data,secret:Bool)throws->String {try write(128) {out in bytes(key) {nas_encode($0,secret,out)}}}
    static func key(_ typed:String)throws->Data {
        var key=Data(count:32)
        let ok=key.withUnsafeMutableBytes {out in text(typed) {nas_key($0,coax_v_u8(data:out.bindMemory(to:UInt8.self).baseAddress,count:32))}}
        guard ok,valid(key) else {key.resetBytes(in:0..<key.count);throw NativeFailure.refused("Use a valid nsec or hex private key")};return key
    }
    static func valid(_ key:Data)->Bool {key.count==32 && key.withUnsafeBytes {oa_nostr_seckey_valid($0.bindMemory(to:UInt8.self).baseAddress!)}==1}
    static func publicKey(_ key:Data)throws->Data {var pub=Data(count:32);let ok=key.withUnsafeBytes {secret in pub.withUnsafeMutableBytes {oa_nostr_pubkey(secret.bindMemory(to:UInt8.self).baseAddress!,$0.bindMemory(to:UInt8.self).baseAddress!)}};guard ok==1 else {throw NostrKey.Failure.unreadable};return pub}
    static func random(_ count:Int)throws->Data {var data=Data(count:count);guard data.withUnsafeMutableBytes({SecRandomCopyBytes(kSecRandomDefault,count,$0.baseAddress!)})==errSecSuccess else {throw NostrKey.Failure.signing};return data}
    static func header(_ key:Data,url:String,body:String,bind:Bool,now:Date=Date())throws->String {
        var aux=try random(32),nonce=try random(16);defer {aux.resetBytes(in:0..<aux.count);nonce.resetBytes(in:0..<nonce.count)}
        return try write(8192) {out in bytes(key) {k in bytes(aux) {a in bytes(nonce) {n in text(url) {u in bytes(Data(body.utf8)) {nas_header(k,a,n,Int64(now.timeIntervalSince1970),u,$0,bind,out)}}}}}}
    }
}

/// Separate login-key inventory. Existing phone identity and wallet records stay intact.
struct NativeLoginKeys {
    var service="com.openagents.app.nostr.login.v1"
    private func query(_ pub:String)->[String:Any] {[kSecClass as String:kSecClassGenericPassword,kSecAttrService as String:service,kSecAttrAccount as String:pub,kSecAttrSynchronizable as String:false]}
    func read(_ pub:String)throws->Data? {var q=query(pub);q[kSecReturnData as String]=true;var value:CFTypeRef?;let status=SecItemCopyMatching(q as CFDictionary,&value);if status==errSecItemNotFound {return nil};guard status==errSecSuccess,let data=value as? Data,NativeAccountCodec.valid(data) else {throw NostrKey.Failure.unavailable(status)};return data}
    func add(_ key:Data,pub:String,account:String)throws {var q=query(pub);q[kSecAttrAccessible as String]=kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly;q[kSecAttrLabel as String]=account;q[kSecValueData as String]=key;let status=SecItemAdd(q as CFDictionary,nil);guard status==errSecSuccess || status==errSecDuplicateItem else {throw NostrKey.Failure.unavailable(status)}}
    func remove(_ pub:String)throws {let status=SecItemDelete(query(pub) as CFDictionary);guard status==errSecSuccess || status==errSecItemNotFound else {throw NostrKey.Failure.unavailable(status)}}
}

@MainActor final class NativeAccountServices:NSObject {
    let auth:AppAuth
    var vault=NativeLoginKeys()
    var send:AppAuth.Send
    var authenticate:()async throws->Void = {let context=LAContext();guard try await context.evaluatePolicy(.deviceOwnerAuthentication,localizedReason:"Open your identity backup") else {throw NativeFailure.refused("Authentication cancelled")}}
    private var privateEpoch=UUID()
    private var input="",backup:Data?,backupPub="",expires:Date?,revealed=false
    private var fields=NSHashTable<UITextField>.weakObjects()
    private var saved="",identity="{}",account="{}",ways="{}",sessions="{}",message=""
    var privateClipboard:Bool {NativeAccountCodec.text(saved) {nas_clipboard($0)}}
    init(auth:AppAuth = .shared,send:@escaping AppAuth.Send=NativeAccountCodec.network) {self.auth=auth;self.send=send;super.init();NotificationCenter.default.addObserver(self,selector:#selector(clear),name:UIApplication.willResignActiveNotification,object:nil);NotificationCenter.default.addObserver(self,selector:#selector(captured),name:UIScreen.capturedDidChangeNotification,object:nil)}
    deinit {NotificationCenter.default.removeObserver(self)}
    func editSecret(_ id:String,_ value:String) {if id=="settings.secret.input" {input=String(value.prefix(256))}}
    func bindSecret(_ field:UITextField,id:String) {
        fields.add(field)
        if id=="settings.secret.input" {return}
        field.isSecureTextEntry = !revealed;field.text=secret(id:id)
    }
    func secret(id:String)->String? {guard let backup,let expires,expires>Date(),!UIScreen.main.isCaptured else{return nil};return id=="settings.secret.hex" ? backup.map {String(format:"%02x",$0)}.joined():try? NativeAccountCodec.encode(backup,secret:true)}
    private func forgetBackup() {if var old=backup {backup=nil;old.resetBytes(in:0..<old.count)}}
    @objc func clear() {privateEpoch=UUID();forgetBackup();backupPub="";expires=nil;revealed=false;input="";for field in fields.allObjects {field.text=nil;field.isSecureTextEntry=true}}
    @objc private func captured() {if UIScreen.main.isCaptured {clear()}}
    private func remember(_ data:Data) {forgetBackup();backup=data;backupPub=(try? NativeAccountCodec.publicKey(data).map {String(format:"%02x",$0)}.joined()) ?? "";expires=Date().addingTimeInterval(60);revealed=false;Task { [weak self] in try? await Task.sleep(for:.seconds(60));guard let self,let expires=self.expires,expires<=Date() else{return};self.clear() }}
    func request(_ path:String,method:String="GET",body:String="")async throws->String {
        guard let url=URL(string:auth.origin+path) else {throw AppAuth.Failure.unreachable}
        var q=URLRequest(url:url,cachePolicy:.reloadIgnoringLocalAndRemoteCacheData,timeoutInterval:30);q.httpMethod=method
        if method=="POST" {q.httpBody=Data(body.utf8);q.setValue("application/json",forHTTPHeaderField:"Content-Type");q.setValue(auth.origin,forHTTPHeaderField:"Origin")}
        q=try await auth.authorize(q);let (data,status)=try await send(q);guard data.count<=131072 else {throw NativeFailure.refused("Account response too large")};let raw=String(decoding:data,as:UTF8.self)
        guard (200..<300).contains(status) else {throw NativeAccountHTTP.refused(status,NativeAccountCodec.part(raw,"error").isEmpty ? "Account request failed":NativeAccountCodec.part(raw,"error"))};return raw
    }
    private func permission()async->String {let settings=await UNUserNotificationCenter.current().notificationSettings();switch settings.authorizationStatus {case .authorized,.provisional,.ephemeral:return "Enabled";case .denied:return "Disabled in iOS settings";default:return "Not enabled"}}
    private func snapshot(refresh:Bool=true)async throws->String {
        if auth.loggedIn && (refresh || saved.isEmpty) {
            saved=try await request("/api/auth/settings/v1");identity=try await request("/api/auth/identity/v1");account=try await request("/api/auth/account");ways=try await request("/api/auth/ways");sessions=try await request("/api/auth/sessions")
        } else if !auth.loggedIn {saved=try NativeAccountCodec.write {nas_initial($0)};identity="{}";account="{}";ways="{}";sessions="{}"}
        let status=await permission()
        return try NativeAccountCodec.write(131072) {out in NativeAccountCodec.text(saved) {s in NativeAccountCodec.text(identity) {i in NativeAccountCodec.text(account) {a in NativeAccountCodec.text(ways) {w in NativeAccountCodec.text(sessions) {d in NativeAccountCodec.text(status) {p in NativeAccountCodec.text(backupPub) {bp in NativeAccountCodec.text(message) {nac_observation(s,i,a,w,d,p,backup != nil,bp,$0,out)}}}}}}}}}
    }
    private func localKey(_ pub:String)throws->Data? {
        if let key=try vault.read(pub) {return key}
        if var key=try NostrKey.existingSecret() {let actual=try NativeAccountCodec.publicKey(key).map {String(format:"%02x",$0)}.joined();if actual==pub {return key};key.resetBytes(in:0..<key.count)};return nil
    }
    private func prove(_ presenter:UIViewController)async throws {
        let raw=try await request("/api/auth/ways")
        for index in 0..<16 {let pub=NativeAccountCodec.part(raw,"keys[\(index)].pubkey");guard !pub.isEmpty else{break};if var key=try localKey(pub) {defer {key.resetBytes(in:0..<key.count)};try await authenticate();let url=try await auth.proofChallenge(purpose:"stepup");guard url.hasPrefix(auth.origin+"/api/auth/step-up?c=") else {throw AppAuth.Failure.unreachable};let header=try NativeAccountCodec.header(key,url:url,body:"",bind:false);_ = try await request(String(url.dropFirst(auth.origin.count)),method:"POST",body:try json(["proof":String(header.dropFirst(6))]));return}}
        let challenge=try await auth.passkeyStepUpChallenge(),ask=PasskeyAsk(anchor:presenter.view.window);try await auth.passkeyStepUp(challenge,try await ask.assert(challenge))
    }
    private func json(_ object:[String:String])throws->String {String(decoding:try JSONSerialization.data(withJSONObject:object,options:.sortedKeys),as:UTF8.self)}
    private func link(_ key:Data)async throws {
        let pub=try NativeAccountCodec.publicKey(key).map {String(format:"%02x",$0)}.joined()
        try vault.add(key,pub:pub,account:auth.tokens?.account ?? "")
        let url=try await auth.proofChallenge(purpose:"link"),header=try NativeAccountCodec.header(key,url:url,body:"",bind:false)
        try await auth.linkNostr(url:url,proof:String(header.dropFirst(6)),label:"iPhone login key")
    }
    func perform(_ args:[String:Any],presenter:UIViewController)async throws->[String:Any] {
        let name=args["action"] as? String ?? "",target=args["target"] as? String ?? "",payload=args["payload"] as? String ?? ""
        let action=NativeAccountCodec.text(name) {nac_action($0)};message=""
        switch action {
        case 1:break
        case 2:saved=try await request("/api/auth/settings/v1",method:"POST",body:payload);message="Saved"
        case 3:message=try await publish()
        case 4:let c=try await auth.passkeyRegisterChallenge(),ask=PasskeyAsk(anchor:presenter.view.window),r=try await ask.register(c);try await auth.passkeyRegister(c,attestation:r.attestation,clientData:r.clientData,prf:r.prfSupported);message="Passkey added"
        case 5:try await prove(presenter);message="Verified"
        case 6:
            try await authenticate();let owner=privateEpoch
            let pub=NativeAccountCodec.part(identity,"pubkey")
            var key:Data
            do {
                let raw=try await request("/api/auth/identity/backup/v1",method:"POST",body:"{}")
                guard NativeAccountCodec.part(raw,"pubkey")==pub else {throw NativeFailure.refused("Backup identity mismatch")}
                key=try NativeAccountCodec.key(NativeAccountCodec.part(raw,"secret"))
            }catch NativeAccountHTTP.refused(409,_) {
                guard let held=try localKey(pub) else {throw NativeFailure.refused("Import this identity’s key on this phone first")};key=held
            }
            defer {key.resetBytes(in:0..<key.count)}
            guard try NativeAccountCodec.publicKey(key).map({String(format:"%02x",$0)}).joined()==pub else {throw NativeFailure.refused("Backup identity mismatch")};guard owner==privateEpoch else {throw NativeFailure.refused("Backup closed")};remember(key)
        case 7:clear()
        case 8,9:guard let value=secret(id:action==8 ? "settings.secret.hex":"settings.secret.nsec") else {throw NativeFailure.refused("Backup expired. Open it again.")};UIPasteboard.general.setItems([[UIPasteboard.typeAutomatic:value]],options:[.localOnly:true,.expirationDate:Date().addingTimeInterval(30)]);message="Copied · expires in 30 seconds"
        case 10:var key=try NativeAccountCodec.key(input);defer {key.resetBytes(in:0..<key.count);input="";for f in fields.allObjects where f.accessibilityIdentifier=="settings.secret.input" {f.text=nil}};try await link(key);message="Login key linked"
        case 11:
            var key=try NativeAccountCodec.key(input);defer {key.resetBytes(in:0..<key.count);clear()};let held=key
            _ = try await auth.keyLogin(signer:{url,body,now in try NativeAccountCodec.header(held,url:url,body:body,bind:true,now:now)})
            let pub=try NativeAccountCodec.publicKey(key).map {String(format:"%02x",$0)}.joined();try vault.add(key,pub:pub,account:auth.tokens?.account ?? "")
        case 12:try await auth.unlinkNostr(pubkey:target);try vault.remove(target);message="Login key removed"
        case 13:break
        case 14:let allowed=try await UNUserNotificationCenter.current().requestAuthorization(options:[.alert,.badge,.sound]);if allowed {UIApplication.shared.registerForRemoteNotifications()};message=allowed ? "Security alerts enabled":"Notifications remain disabled"
        case 15:if let url=URL(string:UIApplication.openSettingsURLString) {_ = await UIApplication.shared.open(url)}
        case 16:_ = try await request("/api/auth/devices/push/off",method:"POST",body:"{}");message="Push disabled for this device"
        case 17:guard NativeAccountCodec.text(target,{nac_target(action,$0)}) else {throw NativeFailure.refused("Invalid session")};_ = try await request("/api/auth/sessions/"+target,method:"POST",body:"{}");message="Session ended";if NativeAccountCodec.text(sessions,{s in NativeAccountCodec.text(target,{nac_current(s,$0)})}) {await auth.logout();clear()}
        case 18:_ = try await request("/api/auth/logout-everywhere",method:"POST",body:"{}");await auth.logout();clear()
        case 19:guard backup != nil else {throw NativeFailure.refused("Open backup first")};revealed.toggle();for f in fields.allObjects where f.accessibilityIdentifier?.hasPrefix("settings.secret.")==true && f.accessibilityIdentifier != "settings.secret.input" {f.isSecureTextEntry = !revealed}
        case 20:var key=try NativeAccountCodec.random(32);defer {key.resetBytes(in:0..<key.count)};guard NativeAccountCodec.valid(key) else {throw NostrKey.Failure.signing};try await link(key);message="Login key linked · saved in Keychain"
        case 21:
            guard NativeAccountCodec.text(target,{nac_target(action,$0)}) else {throw NativeFailure.refused("Invalid key")};try await authenticate()
            guard var key=try localKey(target) else {throw NativeFailure.refused("This login key is held on another device")};defer {key.resetBytes(in:0..<key.count)};remember(key)
        case 22:
            guard NativeAccountCodec.text(target,{nac_target(action,$0)}) else {throw NativeFailure.refused("Invalid passkey")};_ = try await request("/api/auth/unlink",method:"POST",body:"{\"kind\":\"passkey\",\"id\":"+target+"}");message="Passkey removed"
        default:throw NativeFailure.refused("Unsupported account service")
        }
        return ["record":try await snapshot(refresh:![7,8,9,19].contains(Int(action)))]
    }
    private func publish()async throws->String {
        saved=try await request("/api/auth/settings/v1")
        let event:String
        do {event=try await request("/api/auth/settings/profile-event/v1",method:"POST",body:"{}")}
        catch NativeAccountHTTP.refused(409,_) {
            let pub=NativeAccountCodec.part(identity,"pubkey")
            guard var key=try localKey(pub) else {throw NativeFailure.refused("Import this identity’s key on this phone to publish")};defer {key.resetBytes(in:0..<key.count)}
            var aux=try NativeAccountCodec.random(32);defer {aux.resetBytes(in:0..<aux.count)}
            event=try NativeAccountCodec.write(16384) {out in NativeAccountCodec.bytes(key) {k in NativeAccountCodec.bytes(aux) {a in NativeAccountCodec.text(saved) {nas_profile(k,a,Int64(Date().timeIntervalSince1970),$0,out)}}}}
        }
        let id=NativeAccountCodec.part(event,"id")
        let frame=try NativeAccountCodec.write {out in NativeAccountCodec.text(event) {nas_event($0,out)}}
        var sent=0
        for index in 0..<6 {
            let raw=NativeAccountCodec.part(saved,"relays[\(index)]");guard !raw.isEmpty,let url=URL(string:raw) else{break}
            let socket=URLSession.shared.webSocketTask(with:url);socket.resume();defer {socket.cancel(with:.normalClosure,reason:nil)}
            let deadline=Task {try? await Task.sleep(for:.seconds(8));guard !Task.isCancelled else{return};socket.cancel(with:.goingAway,reason:nil)};defer {deadline.cancel()}
            do {try await socket.send(.string(frame));for _ in 0..<8 {let message=try await socket.receive();guard case .string(let text)=message else{continue};let ok=NativeAccountCodec.text(text) {f in NativeAccountCodec.text(id) {nas_accepted(f,$0)}};if ok {sent+=1;break}}}catch {continue}
        }
        guard sent>0 else {throw NativeFailure.refused("Saved. No relay accepted the profile. Retry publish.")};return "Published · \(sent) relay\(sent==1 ? "":"s")"
    }
}
