// Firm reason: 6 (UIKit lifecycle, the keyboard and text input, and VoiceOver).
// The OpenAgents iOS app (components/gpu-components.md; GitHub #13645): one screen, the chat page.
// The page is compiled Coax (chat.coax) drawn by the shared GPU component host in one Metal view.
// Nothing runs in JavaScript. This file owns only what a GPU surface cannot:
//   - the display link, which sleeps while nothing moves and stops in the background;
//   - the platform editors: a UITextView over the composer's text, and a UITextField over a login
//     field, each only while its field holds the keyboard. Which field holds it is the page's
//     (kit/frame.coax's focus rule: a tap outside the focused field takes it away); this file
//     shows the keyboard for that field and hides it when there is none;
//   - VoiceOver: the shared bridge (apps/shared/native/KitAccess.swift) reads the committed semantic
//     rows; the Metal view itself is hidden from accessibility. The platform editor and the system
//     controls stand for their own rows;
//   - large text: the person's Dynamic Type size is the page's text zoom (oc_text_zoom);
//   - the system controls over the page (GlassChrome.swift): the Liquid Glass menu button and the
//     chips' picker as a popover, which the page keeps the rows of and does not paint;
//   - the shared app core with its native services (AppCore.swift), attached to the page.
import UIKit

@main final class OpenAgentsApp:UIResponder,UIApplicationDelegate {
    var window:UIWindow?
    func application(_ application:UIApplication,didFinishLaunchingWithOptions options:[UIApplication.LaunchOptionsKey:Any]?=nil)->Bool {
        let window=UIWindow(frame:UIScreen.main.bounds);window.rootViewController=ChatScreen();window.makeKeyAndVisible();self.window=window;return true
    }
}

@MainActor final class ChatScreen:UIViewController,UITextViewDelegate,UITextFieldDelegate,KitAccessSource {
    private var owner:UnsafeMutableRawPointer?
    private var canvas:UIView?
    private var display:CADisplayLink?
    private var keyboard:CGFloat=0
    private var draftRev:Int32 = -1
    private var access:KitAccess?
    private var zoom:Float=1
    /// The editors' style from the page's skin, at the text size the page draws.
    private func style()->[Float] {var s=[Float](repeating:0,count:14);_ = oc_style(&s);s[0]*=zoom;s[1]*=zoom;return s}
    private lazy var field:FieldEditor=FieldEditor(style:style())
    /// The shared app core and the native services it asks for.
    private var services:AppServices?
    private var chrome:GlassChrome?
    private lazy var editor:ComposerEditor=ComposerEditor(style:style())

    override func viewDidLoad() {
        super.viewDidLoad();view.backgroundColor = .black;view.accessibilityIdentifier="chat.screen"
        guard let resources=Bundle.main.path(forResource:"Components",ofType:nil),let owner=resources.withCString({oc_open($0)}),let raw=oc_view(owner) else {
            let label=UILabel();label.text="OpenAgents could not start. Restart the app.";label.numberOfLines=0;label.textAlignment = .center;label.textColor = .label
            label.translatesAutoresizingMaskIntoConstraints=false;view.addSubview(label)
            NSLayoutConstraint.activate([label.centerYAnchor.constraint(equalTo:view.centerYAnchor),label.leadingAnchor.constraint(equalTo:view.leadingAnchor,constant:24),label.trailingAnchor.constraint(equalTo:view.trailingAnchor,constant:-24)]);return
        }
        self.owner=owner
        services=AppServices()
        if let services {
            services.bridge.changed={ [weak self] in self?.wake() }
            oc_attach(owner,services.core.raw,AppServices.keptDraft)
            oc_env_restore(owner,AppServices.keptEnvironment)
            // after every call into the core: the login flow follows it, and the page draws what changed
            services.core.after={[weak self] in oc_step(owner);self?.wake()}
            services.launch()
        }
        // A touch lands on the Metal view, not here: the surface says when one changed something.
        oc_on_change(owner,{ context in
            guard let context else {return}
            let screen=Unmanaged<ChatScreen>.fromOpaque(context).takeUnretainedValue()
            MainActor.assumeIsolated {screen.wake()}
        },Unmanaged.passUnretained(self).toOpaque())
        let canvas=Unmanaged<UIView>.fromOpaque(raw).takeUnretainedValue();self.canvas=canvas
        canvas.frame=view.bounds;canvas.autoresizingMask=[.flexibleWidth,.flexibleHeight];view.addSubview(canvas)
        canvas.isAccessibilityElement=false;canvas.accessibilityElementsHidden=true
        // the menu button and the chips' picker are system controls over the page
        oc_native(owner,1)
        let chrome=GlassChrome(on:self);self.chrome=chrome
        chrome.activate={ [weak self] id in guard let self,let owner=self.owner else {return false};let pressed=oc_activate(owner,id) != 0;self.wake();return pressed }
        access=makeAccess()
        textSize()
        field.delegate=self;field.addTarget(self,action:#selector(fieldChanged),for:.editingChanged)
        editor.delegate=self;editor.host=view;editor.attach={ [weak self] in self?.attachEditor() };editor.activate={ [weak self] in self?.focus() ?? false }
        let center=NotificationCenter.default
        center.addObserver(self,selector:#selector(keyboardMoved(_:)),name:UIResponder.keyboardWillChangeFrameNotification,object:nil)
        center.addObserver(self,selector:#selector(keyboardHid),name:UIResponder.keyboardWillHideNotification,object:nil)
        center.addObserver(self,selector:#selector(background),name:UIApplication.willResignActiveNotification,object:nil)
        center.addObserver(self,selector:#selector(wake),name:UIApplication.didBecomeActiveNotification,object:nil)
        center.addObserver(self,selector:#selector(wake),name:UIAccessibility.reduceMotionStatusDidChangeNotification,object:nil)
        center.addObserver(self,selector:#selector(textSize),name:UIContentSizeCategory.didChangeNotification,object:nil)
        DevGallery.install(on:self)
    }
    override func viewDidAppear(_ animated:Bool) {
        super.viewDidAppear(animated);wake()
        #if DEBUG
        // Debug builds: `-OADrive SOCKET` (or `tcp:PORT` with `-OADriveToken TOKEN`) lets coax drive play
        // the shared cases on this app (apps/shared/app/cases/INTEGRATION.md).
        if let place=UserDefaults.standard.string(forKey:"OADrive"),!driveStarted,let owner {
            driveStarted=true
            let scale=Float(view.window?.screen.scale ?? UIScreen.main.scale)
            let token=UserDefaults.standard.string(forKey:"OADriveToken") ?? ""
            oc_on_drive_op(owner,{context in
                guard let context else {return}
                let screen=Unmanaged<ChatScreen>.fromOpaque(context).takeUnretainedValue()
                MainActor.assumeIsolated {screen.step();screen.step()}   // the keyboard a tap asked for, then the page sees it
            },Unmanaged.passUnretained(self).toOpaque())
            if oc_drive_start(owner,place,token,Float(view.bounds.width)*scale,Float(view.bounds.height)*scale,scale)==0 {print("OpenAgents: coax drive could not listen on \(place)")}
        }
        // Debug builds: `-OAPress nav.menu,drawer.scrim` presses those rows in turn after launch, as
        // VoiceOver would, so a screen can be reached and captured without a hand on the simulator.
        if let owner,!pressedAtLaunch {DispatchQueue.main.asyncAfter(deadline:.now()+0.8) { [weak self] in self?.chrome?.sample(owner)}}
        // Debug builds: `-OAEdit "press:chat.composer|type:Hi|mark:ni|commit:に|paste:there|print"` plays edits through
        // the platform editor that holds the keyboard, as a keyboard, an input method and Paste would,
        // one step every 0.6 s, then prints what the editor and the page each hold (OA-EDIT lines).
        if let script=UserDefaults.standard.string(forKey:"OAEdit"),!pressedAtLaunch {
            for (i,step) in script.split(separator:"|").enumerated() {
                DispatchQueue.main.asyncAfter(deadline:.now()+1.0+0.6*Double(i)) { [weak self] in self?.edit(String(step)) }
            }
        }
        if let ids=UserDefaults.standard.string(forKey:"OAPress"),!pressedAtLaunch {
            pressedAtLaunch=true
            for (i,id) in ids.split(separator:",").enumerated() {
                DispatchQueue.main.asyncAfter(deadline:.now()+0.8*Double(i+1)) { [weak self] in
                    guard let self,let owner=self.owner else {return}
                    _ = oc_activate(owner,String(id));self.wake()
                }
            }
        }
        #endif
    }
    #if DEBUG
    /// One step of -OAEdit on the editor that holds the keyboard.
    private func edit(_ op:String) {
        guard let owner else {return}
        let parts=op.split(separator:":",maxSplits:1).map(String.init),verb=parts[0],arg=parts.count>1 ? parts[1]:""
        let input:(UIResponder & UITextInput)?=editor.isFirstResponder ? editor:field.isFirstResponder ? field:nil
        switch verb {
        case "press":_ = oc_activate(owner,arg);step();step()
        case "type":input?.insertText(arg)
        case "mark":input?.setMarkedText(arg,selectedRange:NSRange(location:arg.utf16.count,length:0))
        case "commit":input?.setMarkedText(arg,selectedRange:NSRange(location:arg.utf16.count,length:0));input?.unmarkText()
        case "paste":UIPasteboard.general.string=arg;(input as? UIResponder)?.paste(nil)
        case "select":if let input,let start=input.position(from:input.beginningOfDocument,offset:Int(arg) ?? 0) {input.selectedTextRange=input.textRange(from:start,to:start)}
        default:break
        }
        // the platform's change events arrive on their own; the page reads the result on its next frame
        if let t=input as? UITextView {textViewDidChange(t)} else if input===field {fieldChanged()}
        step()
        var page=[CChar](repeating:0,count:17000);_ = oc_text(owner,&page,Int32(page.count))
        var fieldText=[CChar](repeating:0,count:512);_ = oc_field_text(owner,&fieldText,Int32(fieldText.count))
        let held=input===field ? String(cString:fieldText):String(cString:page)
        let text=(input as? UITextView)?.text ?? (input as? UITextField)?.text ?? ""
        print("OA-EDIT \(op) focus \(focusID()) editor \"\(text)\" marked \(input?.markedTextRange != nil) page \"\(held)\" content \((input as? UITextField)?.textContentType?.rawValue ?? "-")")
    }
    private var driveStarted=false
    private var pressedAtLaunch=false
    #endif
    override func viewDidLayoutSubviews() {super.viewDidLayoutSubviews();wake()}
    override func viewSafeAreaInsetsDidChange() {super.viewSafeAreaInsetsDidChange();wake()}
    override var preferredStatusBarStyle:UIStatusBarStyle {.lightContent}

    // MARK: frames

    /// Something changed: draw, and keep drawing while the page says it moves.
    @objc func wake() {
        guard owner != nil,viewIfLoaded?.window != nil,UIApplication.shared.applicationState != .background else {return}
        if display==nil {
            let link=CADisplayLink(target:ChatTick(self),selector:#selector(ChatTick.tick));link.preferredFrameRateRange=CAFrameRateRange(minimum:30,maximum:60,preferred:60)
            link.add(to:.main,forMode:.common);display=link
        }
        display?.isPaused=false
    }
    @objc private func background() {
        display?.invalidate();display=nil
        // the chips' choices, kept for the next launch
        if let owner {var bytes=[CChar](repeating:0,count:8192);_ = oc_env_save(owner,&bytes,Int32(bytes.count));AppServices.keepEnvironment(String(cString:bytes))}
    }
    fileprivate func step() {
        guard let owner else {return}
        // editing: a platform editor holds the keyboard over the focused field, and draws its text
        let insets=view.safeAreaInsets,composing=editor.isFirstResponder,editing=composing||field.isFirstResponder
        oc_env(owner,Float(insets.top),Float(insets.bottom),Float(keyboard),editing ? 1:0,composing ? Float(editor.textHeight(width:editor.bounds.width)):0,UIAccessibility.isReduceMotionEnabled ? 1:0)
        oc_frame(owner)
        services?.pump()
        if oc_action(owner)==1 {_ = focus()}
        followFocus();placeEditor();access?.refresh();keepDraft()
        #if DEBUG
        dumpAccess()
        #endif
        chrome?.sync(owner,rowsStamp:oc_rows_stamp(owner))
        // A static page sleeps: no frame runs until something wakes it.
        if oc_moving(owner)==0 {display?.isPaused=true}
    }

    // MARK: the platform editor

    private func box()->CGRect? {
        guard let owner else {return nil}
        var b=[Float](repeating:0,count:6)
        guard oc_editor(owner,&b) != 0 else {return nil}
        return CGRect(x:CGFloat(b[0]),y:CGFloat(b[1]),width:CGFloat(b[2]),height:CGFloat(b[3]))
    }
    private func attachEditor() {
        if editor.superview==nil {view.addSubview(editor)}
        if let frame=box() {editor.frame=frame}
    }
    private func placeEditor() {
        guard let owner else {return}
        // A sent draft is cleared by the page: the editor follows when it has no marked text.
        var bytes=[CChar](repeating:0,count:17000);_ = oc_text(owner,&bytes,Int32(bytes.count));let held=String(cString:bytes)
        if editor.markedTextRange==nil,editor.text != held {editor.set(held)}
        guard editor.superview != nil,let frame=box() else {return}
        if editor.frame != frame {editor.frame=frame}
        editor.isScrollEnabled=editor.textHeight(width:frame.width)+12>frame.height+1
    }
    /// The field that holds the keyboard, as the page says: its editor takes the keyboard, and with
    /// none the keyboard goes away.
    private func focusID()->String {
        guard let owner else {return ""}
        var bytes=[CChar](repeating:0,count:64);_ = oc_focus(owner,&bytes,Int32(bytes.count));return String(cString:bytes)
    }
    private func followFocus() {
        guard let owner else {return}
        let id=focusID()
        if id != "chat.composer",editor.isFirstResponder {_ = editor.resignFirstResponder()}
        guard id.hasPrefix("login.") else {if field.isFirstResponder {_ = field.resignFirstResponder()};field.removeFromSuperview();return}
        var b=[Float](repeating:0,count:4)
        guard oc_field_box(owner,&b) != 0 else {return}
        let row=CGRect(x:CGFloat(b[0]),y:CGFloat(b[1]),width:CGFloat(b[2]),height:CGFloat(b[3]))
        // the editor sits over the field's text line, in the middle of the field's row
        let frame=CGRect(x:row.minX+11,y:row.midY-min(22,row.height/2),width:max(40,row.width-22),height:min(44,row.height))
        if field.superview==nil {view.addSubview(field)}
        if field.frame != frame {field.frame=frame}
        field.configure(id=="login.code" ? .code:.email,placeholder:id=="login.code" ? "000000":"you@example.com")
        if field.markedTextRange==nil {
            var bytes=[CChar](repeating:0,count:512);_ = oc_field_text(owner,&bytes,Int32(bytes.count));let held=String(cString:bytes)
            if field.text != held {field.text=held}
        }
        if !field.isFirstResponder,!field.becomeFirstResponder() {oc_blur(owner)}
    }
    private func keepDraft() {
        guard let owner else {return}
        let rev=oc_draft_rev(owner)
        guard rev != draftRev else {return}
        draftRev=rev
        var bytes=[CChar](repeating:0,count:600);_ = oc_draft(owner,&bytes,Int32(bytes.count))
        AppServices.keepDraft(String(cString:bytes))
    }
    @objc private func fieldChanged() {
        // an input method's provisional text stays the platform's until it is committed (one editing authority)
        guard let owner,field.markedTextRange==nil else {return}
        if oc_set_field(owner,field.text ?? "")==0 {var bytes=[CChar](repeating:0,count:512);_ = oc_field_text(owner,&bytes,Int32(bytes.count));field.text=String(cString:bytes)}
        wake()
    }
    func textFieldShouldReturn(_ textField:UITextField)->Bool {if let owner {_ = oc_submit(owner)};wake();return false}
    func textFieldDidEndEditing(_ textField:UITextField) {if let owner,focusID().hasPrefix("login.") {oc_blur(owner)};wake()}
    @discardableResult private func focus()->Bool {attachEditor();let took=editor.becomeFirstResponder();wake();return took}
    func textViewDidBeginEditing(_ textView:UITextView) {wake()}
    func textViewDidEndEditing(_ textView:UITextView) {editor.removeFromSuperview();if let owner,focusID()=="chat.composer" {oc_blur(owner)};wake()}
    func textViewDidChange(_ textView:UITextView) {if let owner,textView.markedTextRange==nil {_ = oc_set_text(owner,textView.text ?? "")};wake()}
    @objc private func keyboardMoved(_ note:Notification) {
        guard let end=(note.userInfo?[UIResponder.keyboardFrameEndUserInfoKey] as? NSValue)?.cgRectValue,let window=view.window else {return}
        let inView=view.convert(end,from:window.screen.coordinateSpace)
        keyboard=max(0,view.bounds.maxY-inView.minY);wake()
    }
    @objc private func keyboardHid() {keyboard=0;wake()}

    // MARK: large text

    /// The person's text size (Dynamic Type) against the default, as the page's text zoom.
    @objc private func textSize() {
        guard let owner else {return}
        let size=UITraitCollection(preferredContentSizeCategory:UIApplication.shared.preferredContentSizeCategory)
        let want=Float(UIFontMetrics(forTextStyle:.body).scaledValue(for:17,compatibleWith:size)/17)
        let now=oc_text_zoom(owner,want)
        #if DEBUG
        if UserDefaults.standard.bool(forKey:"OAAccessDump") {print("OA-TEXT-SIZE \(UIApplication.shared.preferredContentSizeCategory.rawValue) wants \(want) draws \(now)")}
        #endif
        guard now != zoom else {return}
        zoom=now
        let s=style();field.restyle(s);editor.restyle(s);wake()
    }

    // MARK: VoiceOver

    var accessStamp:Int32 {owner.map {oc_rows_stamp($0)} ?? 0}
    var accessLayer:Int {owner.map {Int(oc_layer($0))} ?? 0}
    var accessCount:Int {owner.map {Int(oc_rows($0))} ?? 0}
    func accessRow(_ i:Int)->KitRow? {
        guard let owner else {return nil}
        var row=[Float](repeating:0,count:8),id=[CChar](repeating:0,count:64),label=[CChar](repeating:0,count:4100),value=[CChar](repeating:0,count:256)
        guard oc_row(owner,Int32(i),&row,&id,Int32(id.count),&label,Int32(label.count)) != 0 else {return nil}
        _ = oc_row_value(owner,Int32(i),&value,Int32(value.count))
        return KitRow(id:String(cString:id),label:String(cString:label),value:String(cString:value),role:Int(row[4]),state:Int(row[5]),parent:Int(row[6]),focus:Int(row[7]),
            frame:CGRect(x:CGFloat(row[0]),y:CGFloat(row[1]),width:CGFloat(row[2]),height:CGFloat(row[3])))
    }
    func accessActivate(_ id:String)->Bool {guard let owner else {return false};let pressed=oc_activate(owner,id) != 0;wake();return pressed}

    /// The bridge over this page's rows. The platform editors and the system controls stand for their rows.
    private func makeAccess()->KitAccess {
        let access=KitAccess(container:view,source:self)
        access.stand={ [weak self] row in
            guard let self else {return .skip}
            // the system controls stand for their own rows; the picker's popover is SwiftUI's own
            if let chrome=self.chrome,GlassChrome.covers(row.id) {return row.id=="nav.menu" ? .view(chrome.menu):row.id=="login.back" ? .view(chrome.back):.skip}
            if row.id=="chat.composer" {
                // the composer is the platform editor: over the text while it has the keyboard, else standing for the field's row
                self.editor.box=row.frame;self.editor.accessibilityIdentifier=row.id;self.editor.accessibilityLabel=row.label;self.editor.placeholder=row.label
                return .view(self.editor)
            }
            // a login field that holds the keyboard is the platform field itself
            if row.role==KitRole.field,self.field.isFirstResponder,self.focusID()==row.id {
                self.field.accessibilityIdentifier=row.id;self.field.accessibilityLabel=row.label;return .view(self.field)
            }
            return .own
        }
        access.scroll={ [weak self] direction in
            guard let self,let owner=self.owner else {return false}
            let dir:Int32=direction == .up ? 1:direction == .down ? -1:0
            guard dir != 0,oc_scroll_page(owner,dir) != 0 else {return false}
            self.wake();UIAccessibility.post(notification:.pageScrolled,argument:nil);return true
        }
        // VoiceOver's escape: the open picker, the login card or the sidebar closes, as their own close does
        access.escape={ [weak self] in
            guard let self,let owner=self.owner,oc_layer(owner) != 0 else {return false}
            for id in ["env.close","login.back","drawer.scrim"] where oc_activate(owner,id) != 0 {self.wake();return true}
            return false
        }
        #if DEBUG
        access.announced={print("OA-ANNOUNCE \($0)")}
        #endif
        return access
    }
    #if DEBUG
    // Debug builds: `-OAAccessDump YES` prints the VoiceOver elements each time they change, between
    // OA-ACCESS-BEGIN and OA-ACCESS-END lines (xcrun simctl launch --console-pty), so a check can read
    // what VoiceOver reads.
    private var dumped=""
    private func dumpAccess() {
        guard UserDefaults.standard.bool(forKey:"OAAccessDump"),let access else {return}
        let now=access.dump()
        guard now != dumped else {return}
        dumped=now;print("OA-ACCESS-BEGIN layer \(accessLayer) zoom \(zoom)\n\(now)\nOA-ACCESS-END")
    }
    #endif
}

/// The display link's target, so the link does not keep the screen alive.
@MainActor private final class ChatTick:NSObject {
    weak var screen:ChatScreen?
    init(_ screen:ChatScreen) {self.screen=screen}
    @objc func tick() {screen?.step()}
}
