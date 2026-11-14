// Firm reason: 6 (JavaScriptCore execution SDK; UIKit presentation is separate).
import Foundation
import JavaScriptCore

@MainActor final class NativeProgram {
    struct Node {
        var id:String; var tag:String; var text=""; var value=""; var classes=""
        var attrs:[String:String]=[:]; var children:[Int]=[]
    }
    let context:JSContext
    var nodes:[Int:Node]=[:]
    var roots:[Int]=[]
    var focus:Int?
    private var failure:String?
    init(directory:URL,module:String,diagnostic:((String)->Void)?=nil)throws {
        context=JSContext()!
        context.exceptionHandler = { [weak self] _,error in self?.failure="The Coax module failed.";diagnostic?(error?.toString() ?? "Unknown JavaScriptCore exception") }
        let decode:@convention(block) ([NSNumber])->String = { String(decoding:$0.map {UInt8(truncating:$0)},as:UTF8.self) }
        context.setObject(decode,forKeyedSubscript:"cnUTF8" as NSString)
        for path in ["tree-commands.js","build/\(module).js","runtime.js"] {
            let data=try Data(contentsOf:directory.appendingPathComponent(path))
            guard data.count<=8388608,let script=String(data:data,encoding:.utf8) else {throw NativeFailure.refused("Invalid native program")}
            context.evaluateScript(script,withSourceURL:URL(string:"coax-native:///\(path)"))
            if let failure {throw NativeFailure.refused(failure)}
        }
    }
    private func call(_ name:String,_ args:[Any]=[])throws->JSValue {
        guard let f=context.objectForKeyedSubscript(name),let result=f.call(withArguments:args),failure==nil else {throw NativeFailure.refused("The Coax module failed.")}
        return result
    }
    func frame()throws {
        guard let commands=try call("cnFrame").toArray() as? [[Any]],commands.count<=65536 else {throw NativeFailure.refused("Invalid component frame")}
        // Even a signed, incompatible program fails as an update, not a Swift cast.
        for c in commands {
            guard let op=(c.first as? NSNumber)?.intValue else {throw NativeFailure.refused("Invalid component command")}
            let shape:[String]
            switch op {
            case 1:shape=["n","n","s","s"]
            case 12:shape=["n"]
            case 2,11:shape=["n","n"]
            case 3:shape=["n","n","n","n"]
            case 4,5,7,10:shape=["n","n","s"]
            case 6:shape=["n","n","s","s"]
            case 8:shape=["n","n","a"]
            case 9:shape=["n","s"]
            default:throw NativeFailure.refused("Unsupported component command")
            }
            guard c.count==shape.count else {throw NativeFailure.refused("Invalid component command")}
            for (v,t) in zip(c,shape) {
                guard t=="n" && v is NSNumber || t=="s" && v is String || t=="a" && v is [Any] else {throw NativeFailure.refused("Invalid component value")}
            }
        }
        for c in commands {
            let op=(c[0] as! NSNumber).intValue
            if op==12 {nodes=[:];roots=[];focus=nil;continue}
            func num(_ i:Int)->Int {(c[i] as! NSNumber).intValue}
            func str(_ i:Int)->String {c[i] as! String}
            if op==1 {guard nodes.count<4096 else {throw NativeFailure.refused("Too many controls")};nodes[num(1)]=Node(id:str(3),tag:str(2))}
            else if op==2 {let slot=num(1);nodes.removeValue(forKey:slot);detach(slot)}
            else if op==3 {
                let slot=num(1),parent=num(2),after=num(3);detach(slot)
                var list=parent==65535 ? roots : nodes[parent]?.children ?? []
                let at=after==65535 ? 0 : (list.firstIndex(of:after).map {$0+1} ?? list.count)
                list.insert(slot,at:at)
                if parent==65535 {roots=list}else {nodes[parent]?.children=list}
            } else if op==4 {nodes[num(1)]?.text=str(2)}
            else if op==5 {nodes[num(1)]?.classes=str(2)}
            else if op==6 {nodes[num(1)]?.attrs[str(2)]=str(3)}
            else if op==7 {nodes[num(1)]?.attrs.removeValue(forKey:str(2))}
            else if op==10 {nodes[num(1)]?.value=str(2)}
            else if op==11 {focus=num(1)}
            // CSS, classes and vector decoration belong to the web skin. UIKit
            // supplies platform appearance, layout, hit testing and accessibility.
        }
    }
    private func detach(_ slot:Int) {
        roots.removeAll {$0==slot}
        for key in Array(nodes.keys) {nodes[key]?.children.removeAll {$0==slot}}
    }
    func elements(dialog:Bool=false)->[NativeElement] {
        var result:[NativeElement]=[];var seen:Set<Int>=[]
        func text(_ slot:Int,_ path:Set<Int>=[])->String {
            guard !path.contains(slot),let n=nodes[slot] else {return ""}
            var next=path;next.insert(slot)
            return n.text+n.children.map {text($0,next)}.joined()
        }
        func visit(_ slot:Int,_ inside:Bool) {
            guard seen.insert(slot).inserted,let n=nodes[slot] else {return}
            if n.attrs["hidden"] != nil || n.attrs["aria-hidden"]=="true" {return}
            let isDialog=inside || n.attrs["role"]=="dialog"
            if isDialog==dialog {
                if n.classes.split(separator:" ").contains("transcript") {result.append(NativeElement(id:n.id,role:"transcript",label:"Conversation",value:n.attrs["data-state"] ?? "new"))}
                if n.attrs["role"]=="article" || n.attrs["role"]=="toolbar" || n.classes.split(separator:" ").contains("msg-ai") {
                    func semantic(_ key:Int,_ path:Set<Int>=[])->NativeElement? {
                        guard !path.contains(key),let node=nodes[key] else{return nil}
                        var next=path;next.insert(key)
                        return NativeElement(id:node.id,role:node.tag,label:node.text,children:node.children.compactMap {semantic($0,next)},classes:node.classes,attrs:node.attrs)
                    }
                    if var message=semantic(slot) {message.role=n.attrs["role"]=="toolbar" ? "toolbar":"message";result.append(message)}
                    return
                }
                if n.classes.split(separator:" ").contains("composer") {
                    var children:[NativeElement]=[];var collected:Set<Int>=[]
                    func collect(_ key:Int) {guard collected.insert(key).inserted,let c=nodes[key] else{return};if ["textarea","button"].contains(c.tag) {children.append(NativeElement(id:c.id,role:c.tag=="textarea" ? "field":"button",label:c.tag=="textarea" ? c.attrs["aria-label"] ?? c.attrs["placeholder"] ?? "Message":text(key),value:c.value,disabled:c.attrs["disabled"] != nil))}else {for key in c.children {collect(key)}}}
                    for child in n.children {collect(child)}
                    result.append(NativeElement(id:n.id,role:"composer",label:"Message",children:children,attrs:n.attrs));return
                }
                let field=n.tag=="input" || n.tag=="textarea"
                let role=n.tag=="button" ? "button" : field ? "field" : n.tag=="h1" || n.tag=="h2" ? "heading" : "text"
                if field || n.tag=="button" || !n.text.isEmpty {
                    result.append(NativeElement(id:n.id,role:role,label:field ? n.attrs["aria-label"] ?? n.attrs["placeholder"] ?? "" : n.tag=="button" ? text(slot):n.text,value:n.value,disabled:n.attrs["disabled"] != nil,complete:n.attrs["autocomplete"] ?? "",type:n.tag.hasPrefix("h") ? n.tag:n.tag=="textarea" ? "multiline":n.attrs["type"] ?? "",classes:n.classes,attrs:n.attrs))
                }
            }
            if n.tag=="button" || n.tag=="input" {return}
            for child in n.children {visit(child,isDialog)}
        }
        for root in roots {visit(root,false)}
        return result
    }
    func input(_ id:String,_ text:String?=nil)throws {
        guard let (slot,_)=nodes.first(where:{$0.value.id==id}) else {return}
        if let text {nodes[slot]?.value=text}
        try event(text==nil ? 1:2,slot,text ?? "")
    }
    func event(_ kind:Int,_ slot:Int,_ text:String)throws {
        _ = try call("cnEvent",[kind,slot,Array(text.utf8)])
    }
    func action()throws->(Int,String) {
        guard let a=try call("cnAction").toArray(),a.count==2,let op=a[0] as? NSNumber,let body=a[1] as? String else {throw NativeFailure.refused("Invalid service request")}
        return (op.intValue,body)
    }
}
