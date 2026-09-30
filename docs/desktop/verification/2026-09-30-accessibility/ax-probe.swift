// Reads a running window through the Mac's accessibility API, as VoiceOver
// does, and acts on it as VoiceOver's commands do (#10024).
//
//   swift ax-probe.swift PID dump
//   swift ax-probe.swift PID press "Button title"
//   swift ax-probe.swift PID set "Placeholder" "text"
//   swift ax-probe.swift PID focus "Placeholder"
//
// The terminal running it needs Accessibility access (System Settings >
// Privacy & Security > Accessibility).
import ApplicationServices
import Foundation

func attribute(_ element: AXUIElement, _ name: String) -> AnyObject? {
    var value: AnyObject?
    guard AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success else { return nil }
    return value
}

func text(_ element: AXUIElement, _ name: String) -> String? {
    attribute(element, name) as? String
}

func children(_ element: AXUIElement) -> [AXUIElement] {
    (attribute(element, kAXChildrenAttribute) as? [AXUIElement]) ?? []
}

func describe(_ element: AXUIElement) -> String {
    var line = text(element, kAXRoleAttribute) ?? "?"
    for (name, key) in [("title", kAXTitleAttribute), ("description", kAXDescriptionAttribute),
                        ("placeholder", kAXPlaceholderValueAttribute)] {
        if let value = text(element, key), !value.isEmpty { line += " \(name)=\(value.debugDescription)" }
    }
    if let value = attribute(element, kAXValueAttribute) {
        line += " value=\(String(describing: value).debugDescription)"
    }
    if let enabled = attribute(element, kAXEnabledAttribute) as? Bool, !enabled { line += " [disabled]" }
    if let focused = attribute(element, kAXFocusedAttribute) as? Bool, focused { line += " *focused*" }
    return line
}

func walk(_ element: AXUIElement, _ depth: Int, _ visit: (AXUIElement, Int) -> Void) {
    visit(element, depth)
    for child in children(element) { walk(child, depth + 1, visit) }
}

func find(_ root: AXUIElement, _ name: String) -> AXUIElement? {
    var found: AXUIElement?
    walk(root, 0) { element, _ in
        if found == nil, [kAXTitleAttribute, kAXDescriptionAttribute, kAXPlaceholderValueAttribute]
            .contains(where: { text(element, $0) == name }) {
            found = element
        }
    }
    return found
}

let args = CommandLine.arguments
guard args.count >= 3, let pid = Int32(args[1]) else {
    print("usage: ax-probe PID dump|press NAME|set NAME TEXT|focus NAME"); exit(2)
}
let app = AXUIElementCreateApplication(pid)
guard let windows = attribute(app, kAXWindowsAttribute) as? [AXUIElement], let window = windows.first else {
    print("no window"); exit(1)
}
switch args[2] {
case "dump":
    walk(window, 0) { element, depth in print(String(repeating: "  ", count: depth) + describe(element)) }
case "press":
    guard let element = find(window, args[3]) else { print("no \(args[3])"); exit(1) }
    print("press \(args[3]): \(AXUIElementPerformAction(element, kAXPressAction as CFString).rawValue)")
case "set":
    guard let element = find(window, args[3]) else { print("no \(args[3])"); exit(1) }
    print("set \(args[3]): \(AXUIElementSetAttributeValue(element, kAXValueAttribute as CFString, args[4] as CFString).rawValue)")
case "focus":
    guard let element = find(window, args[3]) else { print("no \(args[3])"); exit(1) }
    print("focus \(args[3]): \(AXUIElementSetAttributeValue(element, kAXFocusedAttribute as CFString, kCFBooleanTrue).rawValue)")
default:
    print("unknown command"); exit(2)
}
