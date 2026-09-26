// Native decoding and rendering of Rust Native's bounded view contract.
// Application intents stay opaque; native callbacks return identity only.
import SwiftUI

struct NativeColor: Decodable {
    let red: UInt8
    let green: UInt8
    let blue: UInt8
    let alpha: UInt8

    var color: Color {
        Color(.sRGB, red: Double(red) / 255, green: Double(green) / 255,
              blue: Double(blue) / 255, opacity: Double(alpha) / 255)
    }
}

struct NativeStyle: Decodable {
    let foreground: NativeColor?
    let background: NativeColor?
    let padding_top: String?
    let padding_end: String?
    let padding_bottom: String?
    let padding_start: String?
    let gap: String?
    let weight: String?
    let align: String?

    static func points(_ space: String?) -> CGFloat {
        switch space {
        case "none": return 0
        case "xs": return 4
        case "sm": return 8
        case "md": return 16
        case "lg": return 24
        default: return 0
        }
    }

    var insets: EdgeInsets {
        EdgeInsets(top: Self.points(padding_top), leading: Self.points(padding_start),
                   bottom: Self.points(padding_bottom), trailing: Self.points(padding_end))
    }
}

struct NativeNode: Decodable, Identifiable {
    let key: String
    let style: NativeStyle
    let element: NativeElement
    var id: String { key }
}

indirect enum NativeElement: Decodable {
    case stack(String, [NativeNode])
    case list(String, [NativeNode])
    case text(String, String)
    case button(String, Bool)
    case surface(String, String)

    private enum Keys: String, CodingKey { case kind, props }
    private enum Props: String, CodingKey { case axis, children, label, value, role, enabled, resource }

    init(from decoder: Decoder) throws {
        let object = try decoder.container(keyedBy: Keys.self)
        let kind = try object.decode(String.self, forKey: .kind)
        let props = try object.nestedContainer(keyedBy: Props.self, forKey: .props)
        switch kind {
        case "stack": self = .stack(try props.decode(String.self, forKey: .axis),
                                     try props.decode([NativeNode].self, forKey: .children))
        case "list": self = .list(try props.decode(String.self, forKey: .label),
                                   try props.decode([NativeNode].self, forKey: .children))
        case "text": self = .text(try props.decode(String.self, forKey: .value),
                                   try props.decode(String.self, forKey: .role))
        case "button": self = .button(try props.decode(String.self, forKey: .label),
                                       try props.decode(Bool.self, forKey: .enabled))
        case "surface": self = .surface(try props.decode(String.self, forKey: .resource),
                                         try props.decode(String.self, forKey: .label))
        default:
            throw DecodingError.dataCorruptedError(forKey: .kind, in: object,
                                                    debugDescription: "Unsupported native element")
        }
    }
}

struct NativeView: Decodable {
    let schema: String
    let instance: String
    let revision: UInt64
    let root: NativeNode
}

struct NativeRenderer: View {
    let node: NativeNode
    let revision: UInt64
    let followTarget: String?
    let followChanged: ((Bool) -> Void)?
    var surface: ((String, String) -> AnyView)? = nil
    let activate: (String) -> Void

    var body: some View {
        content
            .padding(node.style.insets)
            .foregroundStyle(node.style.foreground?.color ?? .primary)
            .background(node.style.background?.color ?? .clear)
            .fontWeight(node.style.weight == "bold" ? .bold :
                        node.style.weight == "normal" ? .regular : nil)
            .multilineTextAlignment(node.style.align == "center" ? .center :
                                   node.style.align == "end" ? .trailing : .leading)
    }

    private var content: AnyView {
        switch node.element {
        case let .stack(axis, children):
            if axis == "horizontal" {
                return AnyView(HStack(alignment: .top, spacing: NativeStyle.points(node.style.gap)) {
                    ForEach(children) { child in render(child) }
                })
            }
            return AnyView(VStack(alignment: .leading, spacing: NativeStyle.points(node.style.gap)) {
                ForEach(children) { child in render(child) }
            }.frame(maxWidth: .infinity, alignment: .leading))
        case let .list(label, children):
            return AnyView(NativeList(key: node.key, rows: children, label: label, revision: revision,
                                     followTarget: followTarget, followChanged: followChanged,
                                     surface: surface, activate: activate))
        case let .button(label, enabled):
            return AnyView(Button(label) { activate(node.key) }.disabled(!enabled)
                .accessibilityIdentifier(node.key))
        case let .text(value, role):
            return AnyView(NativeText(key: node.key, value: value, role: role))
        case let .surface(resource, label):
            return surface?(resource, label) ?? AnyView(
                Text("This device cannot display \(label).")
                    .accessibilityIdentifier("\(node.key)-unsupported"))
        }
    }

    private func render(_ child: NativeNode) -> NativeRenderer {
        NativeRenderer(node: child, revision: revision, followTarget: followTarget,
                       followChanged: followChanged, surface: surface, activate: activate)
    }
}

private struct NativeText: View {
    let key: String
    let value: String
    let role: String
    @State private var original = false

    private var text: Text {
        if role == "markdown", !original,
           let parsed = try? AttributedString(markdown: value, options: .init(
               interpretedSyntax: .inlineOnlyPreservingWhitespace)) {
            return Text(parsed)
        }
        return Text(verbatim: value)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            text
                .font(role == "code" ? .system(.body, design: .monospaced) :
                      role == "heading" ? .headline : .body)
                .textSelection(.enabled)
                .fixedSize(horizontal: false, vertical: true)
                .accessibilityAddTraits(role == "heading" ? .isHeader : [])
                .accessibilityIdentifier(key)
            if role == "markdown" {
                Button(original ? "Show formatted text" : "Show original Markdown") {
                    original.toggle()
                }.font(.caption).accessibilityIdentifier("\(key)-source")
            }
        }
        // Viewing untrusted Markdown never opens an external destination.
        .environment(\.openURL, OpenURLAction { _ in .discarded })
    }
}

private struct NativeList: View {
    let key: String
    let rows: [NativeNode]
    let label: String
    let revision: UInt64
    let followTarget: String?
    let followChanged: ((Bool) -> Void)?
    let surface: ((String, String) -> AnyView)?
    let activate: (String) -> Void
    @State private var following = true

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            if followChanged != nil {
                Toggle("Follow new messages", isOn: Binding(get: { following }, set: changeFollowing))
                    .accessibilityHint("Turn off to keep your reading position during refreshes.")
                    .accessibilityIdentifier("\(key)-follow")
            }
            ScrollViewReader { proxy in
                List(rows) { row in
                    NativeRenderer(node: row, revision: revision, followTarget: nil,
                                   followChanged: nil, surface: surface, activate: activate)
                        .id(row.key)
                }
                .listStyle(.plain)
                .accessibilityLabel(label)
                .simultaneousGesture(DragGesture(minimumDistance: 8).onChanged { _ in changeFollowing(false) })
                .onChange(of: revision) { _, _ in follow(proxy) }
                .onChange(of: following) { _, _ in follow(proxy) }
                .onChange(of: followTarget) { _, current in following = current != nil }
                .onAppear { following = followTarget != nil; follow(proxy) }
            }
        }.frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func follow(_ proxy: ScrollViewProxy) {
        guard following, let target = followTarget, rows.contains(where: { $0.key == target }) else { return }
        proxy.scrollTo(target, anchor: .bottom)
    }

    private func changeFollowing(_ enabled: Bool) {
        guard enabled != following else { return }
        following = enabled
        followChanged?(enabled)
    }
}
