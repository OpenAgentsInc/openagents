// Firm reason: 6 (UIKit lifecycle, the keyboard and text input, VoiceOver and the native auth adapter).
// The login's native side (components/gpu-components.md, stage G1; GitHub #13645). The form is
// compiled Coax (apps/shared/login/screen.coax) drawn by the shared GPU component host in one
// Metal view. It is shown two ways:
//   - a card over the chat, when someone who is not logged in sends a message or opens anything
//     that needs the account. The view is clear, the screen dims the chat behind the card, and
//     Cancel or a tap outside closes it;
//   - a demo, from Settings, Components: the form over its live scene (the openagents.com splash).
//     It sends nothing and logs nobody in.
// This file owns only what a GPU surface cannot:
//   - the display link, which sleeps while nothing moves and stops in the background;
//   - the platform editor: one UITextField placed over the focused field's committed bounds. It
//     is the only editing authority; Coax receives each whole value and paints no text or caret
//     under it. The code field takes one-time-code AutoFill;
//   - VoiceOver: an element per semantic row, from the same rows the screen hit-tests. The Metal
//     view itself is hidden from accessibility, and activating an element is the screen's tap;
//   - the account: AppAuth starts the email and verifies the code. Tokens and the pending handle
//     stay here and in AppAuth; none enters Coax.
import UIKit
import CoreText

@MainActor final class NativeLogin: UIViewController,UITextFieldDelegate {
    /// A card over the screen behind it, or the demo of the form over its scene.
    enum Mode {case card,demo}
    private let auth:AppAuth
    private let mode:Mode
    private let wantsScene:Bool
    private let done:()->Void
    private let useKey:()->Void
    private let cancel:()->Void
    private var owner:UnsafeMutableRawPointer?
    private var canvas:UIView?
    private var display:CADisplayLink?
    private var pending=""
    private var workosConfig=AppAuth.WorkOSConfig(enabled:false,clientID:"",providers:[])
    private var pendingWorkOS=false
    private var work:Task<Void,Never>?
    private var keyboard:CGFloat=0
    private var stamp:Int32 = -1
    private var ids:[String]=[]
    private var focused:Int32=0
    private var finished=false
    private var said=""
    private lazy var editor=LoginEditor()
    private var closeControl:UIView?
    private var proxies:[String:LoginFieldProxy]=[:]
    /// Frames this screen presented and frames its scene computed, for checks.
    var presented:UInt64 {owner.map {UInt64(lg_presented($0))} ?? 0}
    var sceneFrames:UInt64 {owner.map {UInt64(lg_scene_frames($0))} ?? 0}
    var hasScene:Bool {owner.map {lg_scene($0) != 0} ?? false}

    init(auth:AppAuth,mode:Mode,scene:Bool=true,done:@escaping ()->Void,useKey:@escaping ()->Void,cancel:@escaping ()->Void) {
        self.auth=auth;self.mode=mode;wantsScene=scene && mode == .demo;self.done=done;self.useKey=useKey;self.cancel=cancel;super.init(nibName:nil,bundle:nil)
    }
    required init?(coder:NSCoder) {fatalError()}

    override func viewDidLoad() {
        super.viewDidLoad();view.accessibilityIdentifier="login.screen"
        #if OA_NEXT
        view.backgroundColor = .clear
        #else
        view.backgroundColor=mode == .card ? .clear:NativeSkin.ground
        #endif
        // The card is modal for VoiceOver: the chat behind it is not read.
        view.accessibilityViewIsModal=true
        guard let resources=Bundle.main.path(forResource:"Components",ofType:nil),let owner=resources.withCString({lg_open($0,wantsScene ? 1:0,mode == .card ? 1:0)}),let raw=lg_view(owner) else {
            // The surface could not open: say so plainly. The app's own login under this one still works.
            let label=UILabel();label.text="Log in is unavailable. Restart OpenAgents.";label.numberOfLines=0;label.textAlignment = .center;label.textColor = NativeSkin.color(NativeSkin.tokens.ink)
            label.translatesAutoresizingMaskIntoConstraints=false;view.addSubview(label)
            NSLayoutConstraint.activate([label.centerYAnchor.constraint(equalTo:view.centerYAnchor),label.leadingAnchor.constraint(equalTo:view.leadingAnchor,constant:24),label.trailingAnchor.constraint(equalTo:view.trailingAnchor,constant:-24)]);return
        }
        self.owner=owner
        #if OA_NEXT
        lg_keys(owner,0)   // the lean app has no key login yet
        #endif
        if mode != .demo {
            Task { [weak self] in
                guard let self,let cfg=try? await self.auth.workosConfig(),!self.finished else {return}
                self.workosConfig=cfg
                if cfg.enabled,let o=self.owner {lg_workos(o,1);self.wake()}
            }
        }
        // A touch lands on the Metal view, not here: the surface says when one changed something.
        lg_on_change(owner,{ context in
            guard let context else {return}
            let screen=Unmanaged<NativeLogin>.fromOpaque(context).takeUnretainedValue()
            MainActor.assumeIsolated {screen.wake()}
        },Unmanaged.passUnretained(self).toOpaque())
        let canvas=Unmanaged<UIView>.fromOpaque(raw).takeUnretainedValue();self.canvas=canvas
        canvas.frame=view.bounds;canvas.autoresizingMask=[.flexibleWidth,.flexibleHeight];view.addSubview(canvas)
        // The GPU view is not an accessibility element; the rows are (refreshRows).
        canvas.isAccessibilityElement=false;canvas.accessibilityElementsHidden=true
        editor.delegate=self;editor.addTarget(self,action:#selector(edited),for:.editingChanged)
        #if !OA_NEXT
        if mode == .demo {
            // The demo is left by its own control: nothing on the form closes it.
            let close=NativeButton(.icon);close.glass=true;close.show("Close demo",symbol:"xmark");close.accessibilityIdentifier="login.demo.close"
            close.addAction(UIAction { [weak self] _ in self?.cancel() },for:.primaryActionTriggered)
            close.translatesAutoresizingMaskIntoConstraints=false;view.addSubview(close);closeControl=close
            NSLayoutConstraint.activate([close.leadingAnchor.constraint(equalTo:view.safeAreaLayoutGuide.leadingAnchor,constant:12),close.topAnchor.constraint(equalTo:view.safeAreaLayoutGuide.topAnchor),close.widthAnchor.constraint(equalToConstant:44),close.heightAnchor.constraint(equalToConstant:44)])
        }
        #endif
        let center=NotificationCenter.default
        center.addObserver(self,selector:#selector(keyboardMoved(_:)),name:UIResponder.keyboardWillChangeFrameNotification,object:nil)
        center.addObserver(self,selector:#selector(keyboardHid),name:UIResponder.keyboardWillHideNotification,object:nil)
        center.addObserver(self,selector:#selector(background),name:UIApplication.willResignActiveNotification,object:nil)
        center.addObserver(self,selector:#selector(active),name:UIApplication.didBecomeActiveNotification,object:nil)
        center.addObserver(self,selector:#selector(wake),name:UIAccessibility.reduceMotionStatusDidChangeNotification,object:nil)
    }
    override func viewDidAppear(_ animated:Bool) {
        super.viewDidAppear(animated);wake()
        #if DEBUG
        startStats()
        #endif
    }
    override func viewWillDisappear(_ animated:Bool) {super.viewWillDisappear(animated);stop()}
    override func viewDidLayoutSubviews() {super.viewDidLayoutSubviews();wake()}
    override func viewSafeAreaInsetsDidChange() {super.viewSafeAreaInsetsDidChange();wake()}
    override var preferredStatusBarStyle:UIStatusBarStyle {.lightContent}

    /// Reduce Motion, or a debug build's -OAReduceMotion YES for checks.
    static var reducedMotion:Bool {
        #if DEBUG
        if UserDefaults.standard.bool(forKey:"OAReduceMotion") {return true}
        #endif
        return UIAccessibility.isReduceMotionEnabled
    }

    // MARK: frames

    /// Something changed: draw, and keep drawing while the screen says it moves.
    @objc func wake() {
        guard owner != nil,!finished,viewIfLoaded?.window != nil,UIApplication.shared.applicationState != .background else {return}
        if display==nil {
            let link=CADisplayLink(target:LoginTick(self),selector:#selector(LoginTick.tick));link.preferredFrameRateRange=CAFrameRateRange(minimum:30,maximum:60,preferred:60)
            link.add(to:.main,forMode:.common);display=link
        }
        display?.isPaused=false
    }
    private func stop() {display?.invalidate();display=nil}
    @objc private func background() {stop()}
    @objc private func active() {wake()}
    fileprivate func frame() {
        guard let owner,!finished else {return}
        let insets=view.safeAreaInsets
        lg_env(owner,Float(insets.top),Float(insets.bottom),Float(keyboard),Self.reducedMotion ? 1:0)
        lg_frame(owner)
        act(lg_action(owner));placeEditor();refreshRows()
        // A static screen sleeps: no frame runs until something wakes it.
        if lg_moving(owner)==0 {display?.isPaused=true}
    }
    #if DEBUG
    /// Checks read what was drawn: -OALoginStats PATH writes the frame counts there once a second.
    private var statsTimer:Timer?
    private func startStats() {
        guard statsTimer==nil,let path=UserDefaults.standard.string(forKey:"OALoginStats"),!path.isEmpty else {return}
        statsTimer=Timer.scheduledTimer(withTimeInterval:0.5,repeats:true) { [weak self] _ in
            Task { @MainActor [weak self] in
                guard let self,let owner=self.owner else {return}
                let line="scene=\(lg_scene(owner)) presented=\(lg_presented(owner)) sceneFrames=\(lg_scene_frames(owner)) reduced=\(Self.reducedMotion ? 1:0) moving=\(lg_moving(owner))"
                try? line.write(toFile:path,atomically:true,encoding:.utf8)
            }
        }
    }
    #endif

    // MARK: the account

    private func act(_ action:Int32) {
        guard owner != nil,action != 0 else {return}
        if action==4 {editor.resignFirstResponder();cancel();return}
        if mode == .demo {
            // The demo talks to nobody: the email step moves on, the code is never accepted.
            if action==1 {reply(1,true,"")}else if action==2 {reply(2,false,"This is a demo. Nothing was sent.")}
            return
        }
        if action==3 {editor.resignFirstResponder();useKey();return}
        work?.cancel()
        if action==1 {
            let email=text(1)
            let useWorkOS=workosConfig.enabled
            work=Task { [weak self] in
                do {
                    let handle=useWorkOS ? (try await self?.auth.workosMagicStart(email) ?? "") : (try await self?.auth.emailStart(email) ?? "")
                    guard !Task.isCancelled,let self else {return}
                    self.pending=handle;self.pendingWorkOS=useWorkOS;self.reply(1,true,"")
                }
                catch {guard !Task.isCancelled else {return};self?.reply(1,false,error.localizedDescription)}
            }
        } else if action==2 {
            let code=text(2),handle=pending,useWorkOS=pendingWorkOS
            work=Task { [weak self] in
                do {
                    if useWorkOS {
                        guard let res=try await self?.auth.workosMagicVerify(code:code,pending:handle) else {return}
                        guard !Task.isCancelled,let self else {return}
                        await self.handleWorkOSResult(res,action:2)
                    } else {
                        _ = try await self?.auth.emailVerify(code:code,pending:handle);guard !Task.isCancelled,let self else {return}
                        guard self.auth.loggedIn else {self.reply(2,false,"The login could not be saved on this device. Try again.");return}
                        self.reply(2,true,"");self.finish()
                    }
                }
                catch {guard !Task.isCancelled else {return};self?.reply(2,false,error.localizedDescription)}
            }
        } else if action==5 || action==6 || action==7 || action==8 {
            let cid=workosConfig.clientID,emailHint=text(1).trimmingCharacters(in:.whitespacesAndNewlines)
            work=Task { [weak self] in
                guard let self else {return}
                do {
                    var provider:String?=nil;var connID:String?=nil
                    if action==5 {provider="AppleOAuth"}
                    else if action==6 {provider="GoogleOAuth"}
                    else if action==7 {provider="GitHubOAuth"}
                    else {
                        guard !emailHint.isEmpty else {self.reply(action,false,"Enter your work email first, then tap SSO.");return}
                        let disc=try await self.auth.workosDiscoverSSO(email:emailHint)
                        guard disc.sso,!disc.connectionID.isEmpty else {self.reply(action,false,"No Enterprise SSO found for that email domain.");return}
                        connID=disc.connectionID
                    }
                    let pkce=try AppAuth.workosPKCEStart(clientID:cid,provider:provider,connectionID:connID,loginHint:emailHint.isEmpty ? nil:emailHint)
                    let code=try await AppAuth.workosWebAuthorize(pkce:pkce,anchor:self.view.window)
                    let res=try await self.auth.workosExchangePKCE(code:code,pkce:pkce)
                    guard !Task.isCancelled else {return}
                    await self.handleWorkOSResult(res,action:action)
                } catch {
                    guard !Task.isCancelled else {return}
                    let msg=error.localizedDescription
                    self.reply(action,false,msg=="Sign-in cancelled." ? "":msg)
                }
            }
        } else if action==9 {
            editor.resignFirstResponder()
            let emailHint=text(1).trimmingCharacters(in:.whitespacesAndNewlines)
            let alert=UIAlertController(title:"Email & Password",message:"Sign in or create an account.",preferredStyle:.alert) // stock: system secure-text email+password alert
            alert.addTextField { tf in tf.placeholder="you@example.com";tf.keyboardType = .emailAddress;tf.autocapitalizationType = .none;tf.text=emailHint }
            alert.addTextField { tf in tf.placeholder="Password";tf.isSecureTextEntry=true }
            let submit:(Bool)->Void={ [weak self,weak alert] reg in
                guard let self,let fields=alert?.textFields else {return}
                let addr=(fields[0].text ?? "").trimmingCharacters(in:.whitespacesAndNewlines)
                let pw=fields[1].text ?? ""
                fields[1].text=""
                guard !addr.isEmpty else {self.reply(9,false,"Enter your email address.");return}
                guard !pw.isEmpty else {self.reply(9,false,"Enter your password.");return}
                self.work=Task { [weak self] in
                    guard let self else {return}
                    do {
                        let res=reg ? (try await self.auth.workosPasswordRegister(email:addr,password:pw)) : (try await self.auth.workosPasswordLogin(email:addr,password:pw))
                        guard !Task.isCancelled else {return}
                        await self.handleWorkOSResult(res,action:9)
                    } catch {
                        guard !Task.isCancelled else {return}
                        self.reply(9,false,error.localizedDescription)
                    }
                }
            }
            alert.addAction(UIAlertAction(title:"Log In",style:.default) { _ in submit(false) })
            alert.addAction(UIAlertAction(title:"Create Account",style:.default) { _ in submit(true) })
            alert.addAction(UIAlertAction(title:"Reset Password",style:.default) { [weak self,weak alert] _ in
                guard let self else {return}
                let addr=(alert?.textFields?.first?.text ?? "").trimmingCharacters(in:.whitespacesAndNewlines)
                guard !addr.isEmpty else {self.reply(9,false,"Enter your email address.");return}
                self.work=Task { [weak self] in
                    guard let self else {return}
                    do {
                        try await self.auth.workosPasswordReset(email:addr)
                        guard !Task.isCancelled else {return}
                        self.reply(9,false,"If an account exists for that email, we sent a password reset link.")
                    } catch {
                        guard !Task.isCancelled else {return}
                        self.reply(9,false,error.localizedDescription)
                    }
                }
            })
            alert.addAction(UIAlertAction(title:"Cancel",style:.cancel) { [weak self] _ in self?.reply(9,false,"") })
            present(alert,animated:true)
        }
    }
    private func handleWorkOSResult(_ res:AppAuth.WorkOSAuthResult,action:Int32) async {
        switch res {
        case .loggedIn:
            guard auth.loggedIn else {reply(action,false,"The login could not be saved on this device. Try again.");return}
            reply(action,true,"");finish()
        case .needOrg(let orgPending,let orgs):
            guard !orgs.isEmpty else {reply(action,false,"No organization available for this account.");return}
            let picked:String?=await withCheckedContinuation { cont in
                let sheet=UIAlertController(title:"Choose an organization",message:nil,preferredStyle:.actionSheet) // stock: system organization picker action sheet
                for org in orgs {sheet.addAction(UIAlertAction(title:org.name,style:.default) { _ in cont.resume(returning:org.id) })}
                sheet.addAction(UIAlertAction(title:"Cancel",style:.cancel) { _ in cont.resume(returning:nil) })
                if let pop=sheet.popoverPresentationController {pop.sourceView=self.view;pop.sourceRect=CGRect(x:self.view.bounds.midX,y:self.view.bounds.midY,width:1,height:1)}
                self.present(sheet,animated:true)
            }
            guard let orgID=picked else {reply(action,false,"");return}
            do {
                _ = try await auth.workosSelectOrg(pending:orgPending,organizationID:orgID)
                guard auth.loggedIn else {reply(action,false,"The login could not be saved on this device. Try again.");return}
                reply(action,true,"");finish()
            } catch {reply(action,false,error.localizedDescription)}
        case .verifyEmail(let emailPending):
            let entered:String?=await withCheckedContinuation { cont in
                let alert=UIAlertController(title:"Verify your email",message:"Enter the 6-digit verification code sent to your email.",preferredStyle:.alert) // stock: system email verification code alert
                alert.addTextField { tf in tf.placeholder="123456";tf.keyboardType = .numberPad }
                alert.addAction(UIAlertAction(title:"Verify",style:.default) { _ in cont.resume(returning:alert.textFields?.first?.text) })
                alert.addAction(UIAlertAction(title:"Cancel",style:.cancel) { _ in cont.resume(returning:nil) })
                self.present(alert,animated:true)
            }
            guard let code=entered?.trimmingCharacters(in:.whitespacesAndNewlines),!code.isEmpty else {reply(action,false,"");return}
            do {
                let next=try await auth.workosMagicVerify(code:code,pending:emailPending)
                await handleWorkOSResult(next,action:action)
            } catch {reply(action,false,error.localizedDescription)}
        }
    }
    private func reply(_ action:Int32,_ ok:Bool,_ message:String) {guard let owner else {return};lg_reply(owner,action,ok ? 1:0,message);wake()}
    private func text(_ field:Int32)->String {
        guard let owner else {return ""}
        var bytes=[CChar](repeating:0,count:512);_ = lg_text(owner,field,&bytes,Int32(bytes.count));return String(cString:bytes)
    }
    private func finish() {guard !finished else {return};editor.resignFirstResponder();done()}
    /// Stops drawing and frees the surface, the atlas and the scene. The screen cannot be shown again.
    func close() {
        guard !finished else {return};finished=true;work?.cancel();stop();NotificationCenter.default.removeObserver(self)
        #if DEBUG
        statsTimer?.invalidate();statsTimer=nil
        #endif
        editor.delegate=nil;editor.removeFromSuperview();view.accessibilityElements=nil;proxies=[:]
        canvas?.removeFromSuperview();canvas=nil
        if let owner {lg_on_change(owner,nil,nil);lg_close(owner);self.owner=nil}
    }
    deinit {display?.invalidate();if let owner {lg_close(owner)}}

    // MARK: the platform editor

    private func placeEditor() {
        guard let owner else {return}
        let field=lg_focus(owner)
        var box=[Float](repeating:0,count:5)
        if field != 0,lg_field(owner,field,&box) != 0 {
            let inset=CGFloat(box[4]),frame=CGRect(x:CGFloat(box[0])+inset,y:CGFloat(box[1]),width:CGFloat(box[2])-2*inset,height:CGFloat(box[3]))
            if field != focused {
                focused=field;editor.configure(code:field==2);editor.text=text(field)
                if editor.superview==nil {view.addSubview(editor)}
                editor.frame=frame;editor.becomeFirstResponder()
                UIAccessibility.post(notification:.layoutChanged,argument:editor)
            } else {
                if editor.frame != frame {editor.frame=frame}
                // Every edit goes to the screen at once, so a difference is the screen's doing (a refused code is cleared).
                let held=text(field);if editor.markedTextRange==nil,editor.text != held {editor.text=held}
            }
        } else if focused != 0 {
            focused=0
            if editor.isFirstResponder {editor.resignFirstResponder()}
            editor.removeFromSuperview()
        }
    }
    @objc private func edited() {guard let owner,focused != 0 else {return};lg_set_text(owner,focused,editor.text ?? "");wake()}
    func textFieldShouldReturn(_ textField:UITextField)->Bool {if let owner {lg_submit(owner);wake()};return false}
    func textFieldDidEndEditing(_ textField:UITextField) {
        // The keyboard left on its own (a swipe, an external keyboard's Escape): the field no longer has it.
        guard let owner,focused != 0,lg_focus(owner)==focused else {return}
        lg_blur(owner);wake()
    }
    func textField(_ textField:UITextField,shouldChangeCharactersIn range:NSRange,replacementString string:String)->Bool {
        let now=(textField.text ?? "") as NSString
        return now.replacingCharacters(in:range,with:string).utf8.count <= (focused==2 ? 32:254)
    }
    @objc private func keyboardMoved(_ note:Notification) {
        guard let end=(note.userInfo?[UIResponder.keyboardFrameEndUserInfoKey] as? NSValue)?.cgRectValue,let window=view.window else {return}
        let inView=view.convert(end,from:window.screen.coordinateSpace);_ = window
        keyboard=max(0,view.bounds.maxY-inView.minY);wake()
    }
    @objc private func keyboardHid() {keyboard=0;wake()}

    // MARK: VoiceOver

    /// Builds the accessibility elements from the committed semantic rows when any row changed.
    private func refreshRows() {
        guard let owner else {return}
        let now=lg_rows_stamp(owner)
        guard now != stamp else {return}
        stamp=now
        var elements:[Any]=[],names:[String]=[],notice=""
        for i in 0..<lg_rows(owner) {
            var row=[Float](repeating:0,count:7),id=[CChar](repeating:0,count:64),label=[CChar](repeating:0,count:256),value=[CChar](repeating:0,count:256)
            guard lg_row(owner,i,&row,&id,Int32(id.count),&label,Int32(label.count),&value,Int32(value.count)) != 0 else {continue}
            let name=String(cString:id),words=String(cString:label),state=Int(row[5]),role=Int(row[4])
            // The row's role and state are the semantic table's (components/core/elements.coax).
            let disabled=state & 1 != 0,isFocused=state & 64 != 0,live=state & 1024 != 0
            if role==4 {continue}   // a region groups its rows; VoiceOver reads the rows
            let box=CGRect(x:CGFloat(row[0]),y:CGFloat(row[1]),width:CGFloat(row[2]),height:CGFloat(row[3]))
            names.append(name)
            if role==3 {
                // A field: the platform editor while it has the keyboard, else a text field that is never shown.
                if isFocused,editor.superview != nil {
                    editor.accessibilityIdentifier=name;editor.accessibilityLabel=words;elements.append(editor);continue
                }
                let proxy=proxies[name] ?? LoginFieldProxy()
                proxies[name]=proxy;proxy.host=view;proxy.box=box;proxy.accessibilityIdentifier=name;proxy.accessibilityLabel=words
                proxy.text=String(cString:value);proxy.isEnabled = !disabled
                proxy.activate={ [weak self] in self?.activate(name) ?? false }
                elements.append(proxy);continue
            }
            let element=LoginRow(accessibilityContainer:view)
            element.accessibilityIdentifier=name;element.accessibilityLabel=words;element.accessibilityFrameInContainerSpace=box
            var traits:UIAccessibilityTraits=role==0 ? .button:.staticText
            if role==9 {traits.insert(.header)}
            if disabled {traits.insert(.notEnabled)}
            if live {notice=words}
            element.accessibilityTraits=traits
            if role==0 {element.activate={ [weak self] in self?.activate(name) ?? false }}
            elements.append(element)
        }
        if let closeControl {elements.insert(closeControl,at:0)}
        view.accessibilityElements=elements
        if names != ids {ids=names;UIAccessibility.post(notification:.layoutChanged,argument:nil)}
        if notice != said {said=notice;if !notice.isEmpty {UIAccessibility.post(notification:.announcement,argument:notice)}}
    }
    private func activate(_ id:String)->Bool {
        guard let owner else {return false}
        let pressed=lg_activate(owner,id) != 0;wake();return pressed
    }
}

/// The display link's target, so the link does not keep the screen alive.
@MainActor private final class LoginTick:NSObject {
    weak var screen:NativeLogin?
    init(_ screen:NativeLogin) {self.screen=screen}
    @objc func tick() {screen?.frame()}
}

/// A semantic row as VoiceOver sees it. Activation is the screen's own tap.
private final class LoginRow:UIAccessibilityElement {
    var activate:(()->Bool)?
    override func accessibilityActivate()->Bool {activate?() ?? false}
}

/// A field that does not hold the keyboard, as VoiceOver sees it: a text field that is never in
/// the view hierarchy and never edits. Activating it gives the field the keyboard, and the
/// platform editor takes its place.
private final class LoginFieldProxy:UITextField {
    weak var host:UIView?
    var box=CGRect.zero
    var activate:(()->Bool)?
    override var isAccessibilityElement:Bool {get {true} set {}}
    override var accessibilityFrame:CGRect {get {host.map {UIAccessibility.convertToScreenCoordinates(box,in:$0)} ?? .zero} set {}}
    override var accessibilityValue:String? {get {text} set {}}
    override func accessibilityActivate()->Bool {activate?() ?? false}
    override var canBecomeFirstResponder:Bool {false}
}

/// The one platform editor, styled from the skin's exports (login_editor): the kit's own face
/// at the size the screen draws it, the skin's ink, placeholder and caret.
private final class LoginEditor:UITextField {
    private static let style:[Float] = {var s=[Float](repeating:0,count:14);_ = lg_editor(&s);return s}()
    private static func color(_ at:Int)->UIColor {NativeKitText.color(style,at)}
    private static let face=NativeKitText.face(CGFloat(style[0]>0 ? style[0]:17))
    init() {
        super.init(frame:.zero)
        borderStyle = .none;backgroundColor = .clear;font=Self.face;textColor=Self.color(2);tintColor=Self.color(10)
        autocapitalizationType = .none;autocorrectionType = .no;spellCheckingType = .no;smartDashesType = .no;smartQuotesType = .no
        keyboardAppearance = .dark;enablesReturnKeyAutomatically=true;contentVerticalAlignment = .center
    }
    required init?(coder:NSCoder) {fatalError()}
    func configure(code:Bool) {
        keyboardType=code ? .numberPad:.emailAddress
        textContentType=code ? .oneTimeCode:.emailAddress
        returnKeyType=code ? .go:.send
        attributedPlaceholder=NSAttributedString(string:code ? "000000":"you@example.com",attributes:[.foregroundColor:Self.color(6),.font:Self.face])
        reloadInputViews()
    }
}
