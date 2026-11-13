// Firm reason: 6 (real NSEvent construction and production NSView dispatch).
import AppKit
import Foundation

@main
struct MetalAppKitInputs {
    @MainActor static func main() throws {
        guard CommandLine.arguments.count == 5 else { fatalError("MetalAppKitInputs RESOURCES FIXTURE PLAN RESULT") }
        let plan = try JSONSerialization.jsonObject(with:Data(contentsOf:URL(fileURLWithPath:CommandLine.arguments[3]))) as! [String:Any]
        guard plan["schema"] as? String == "aethyr.metal.appkit-events.v1", let host = mf_open(CommandLine.arguments[1],CommandLine.arguments[2],false,true) else { fatalError("fixture open") }
        _ = NSApplication.shared
        let view = EditorMetalWorld(frame:NSRect(x:0,y:0,width:320,height:180));view.host=host
        defer { view.close() }
        var observations: [[String:Any]] = []
        for op in plan["operations"] as? [[String:Any]] ?? [] {
            let flags = NSEvent.ModifierFlags(rawValue:UInt((op["flags"] as? NSNumber)?.uint64Value ?? 0))
            switch op["op"] as? String {
            case "key", "flags":
                let down = op["down"] as? Bool ?? false
                let type: NSEvent.EventType = op["op"] as? String == "flags" ? .flagsChanged : down ? .keyDown : .keyUp
                guard let event = NSEvent.keyEvent(with:type,location:.zero,modifierFlags:flags,timestamp:0,windowNumber:0,context:nil,characters:op["text"] as? String ?? "",charactersIgnoringModifiers:op["plain"] as? String ?? "",isARepeat:false,keyCode:UInt16((op["code"] as? NSNumber)?.intValue ?? 0)) else { fatalError("NSEvent key") }
                if type == .flagsChanged { view.flagsChanged(with:event) } else if down { view.keyDown(with:event) } else { view.keyUp(with:event) }
            case "wheel":
                let precise = op["precise"] as? Bool ?? false
                guard let cg = CGEvent(scrollWheelEvent2Source:nil,units:precise ? .pixel:.line,wheelCount:1,wheel1:(op["delta"] as? NSNumber)?.int32Value ?? 0,wheel2:0,wheel3:0), let event = NSEvent(cgEvent:cg) else { fatalError("NSEvent wheel") }
                observations.append(["precise":event.hasPreciseScrollingDeltas,"deltaY":event.deltaY,"scrollingDeltaY":event.scrollingDeltaY])
                view.scrollWheel(with:event)
            case "blur": view.focus(false)
            default: fatalError("unknown AppKit event")
            }
        }
        let rows = (0..<mf_observe(host,-1,2)).map { e in (0..<8).map { mf_observe(host,e,Int64($0)) } }
        let result: [String:Any] = ["schema":"aethyr.metal.appkit-observations.v1","events":rows,"wheel":observations,"held":mf_observe(host,-1,5),"dropped":mf_observe(host,-1,3)]
        try JSONSerialization.data(withJSONObject:result,options:[.sortedKeys]).write(to:URL(fileURLWithPath:CommandLine.arguments[4]),options:.atomic)
    }
}
