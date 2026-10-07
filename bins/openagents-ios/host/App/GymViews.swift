// The Gym in chat (wireframe revision 3): the main menu, the first run, the
// cards a chat reply carries, and the sheets over them. Rust builds every
// value here with the app's own words and mints every button's ID; this
// host draws them in the menu's black-and-white style and sends back only
// the ID of the button tapped (`gym`). Nothing here reads a label to decide
// anything, and no number is made up: a value Rust hasn't read is absent.
import SwiftUI
import UIKit

// MARK: - Packet

struct GymButton: Decodable, Equatable, Identifiable {
    let id: String
    let label: String
    let glyph: String?
    let enabled: Bool
}

struct GymLine: Decodable, Equatable {
    let text: String
    /// `body`, `strong`, or `quiet`.
    let tone: String
}

struct GymItem: Decodable, Equatable {
    /// `check`, `cross`, `wait`, `dot`, or `none`; two on a test row.
    let marks: [String]
    let text: String
    let detail: String?
    let trailing: String?
}

struct GymCompare: Decodable, Equatable {
    let without_label: String
    let without: String?
    let with_label: String
    let with: String
}

struct GymProgress: Decodable, Equatable {
    let label: String
    let done: Int
    let total: Int
}

struct GymCard: Decodable, Equatable, Identifiable {
    let id: String
    let kind: String
    let step: String?
    let icon: String?
    let title: String
    let badge: String?
    let compare: GymCompare?
    let lines: [GymLine]
    let items: [GymItem]
    let progress: [GymProgress]
    let primary: GymButton?
    let secondary: [GymButton]
    let chips: [GymButton]
    let source: String?
    let busy: Bool
}

struct GymSection: Decodable, Equatable {
    let heading: String?
    let lines: [GymLine]
    let items: [GymItem]
}

struct GymBar: Decodable, Equatable {
    let label: String
    let value: Int
    let max: Int
}

struct GymSheet: Decodable, Equatable, Identifiable {
    let id: String
    /// `result`, `publish`, `test_set`, `level_up`, `profile`, or `stop`.
    let kind: String
    let title: String
    let headline: String?
    let big: String?
    let compare: GymCompare?
    let sections: [GymSection]
    let bar: GymBar?
    let next: String?
    let primary: GymButton?
    let secondary: [GymButton]
    let close: GymButton?
    let busy: Bool
}

struct GymPlayer: Decodable, Equatable {
    let name: String
    let level: Int?
    let xp_label: String?
    let bar_value: Int
    let bar_max: Int
}

struct GymMenuRow: Decodable, Equatable {
    let button: GymButton
    let subtitle: String
    let glyph: String
}

struct GymMenu: Decodable, Equatable {
    let player: GymPlayer
    let status: String
    let next: String
    let primary: GymButton
    let primary_subtitle: String
    let chips: [GymButton]
    let rows: [GymMenuRow]
    let footer: String
}

struct GymFirstRun: Decodable, Equatable {
    /// `choose` or `end_card`.
    let step: String
    let indicator: String?
    let dot: Int
    let title: String
    let lines: [String]
    /// The agent card: its name and one line.
    let agent: [String]?
    let next: String
    let primary: GymButton
    let secondary: GymButton?
}

struct GymPacket: Decodable, Equatable {
    /// `first_run`, `menu`, or `chat`.
    let screen: String
    let first_run: GymFirstRun?
    let menu: GymMenu
    let cards: [String: GymCard]
    let sheet: GymSheet?
    let share: String?
    let live: Bool
}

/// The packet's `gym` field. A value this build can't read leaves the
/// rest of the app packet readable.
struct GymSlot: Decodable {
    let value: GymPacket?

    init(from decoder: Decoder) throws {
        value = try? GymPacket(from: decoder)
    }
}

// MARK: - Style

/// The menu's style: black, white, and gray; the one primary is white.
enum GymStyle {
    static let surface = Color(white: 0.055)
    static let raised = Color(white: 0.10)
    static let card = Color(white: 0.07)
    static let stroke = Color(white: 1, opacity: 0.16)
    static let strokeStrong = Color(white: 1, opacity: 0.55)
    static let secondary = Color(white: 0.64)
    static let tertiary = Color(white: 0.44)
    static let rowTitle = Font.paper(22, weight: .heavy)
    static let cardTitle = Font.paper(20, weight: .heavy)
    static let cardHeadline = Font.paper(26, weight: .black)
    static let cardNumber = Font.paper(30, weight: .black)
    static let headline = Font.paper(34, weight: .black)
    static let hugeNumber = Font.paper(44, weight: .black)
    static let levelNumber = Font.paper(120, weight: .black)
    static let button = Font.paper(20, weight: .heavy)
    static let section = Font.paper(14, weight: .bold)
    static let body = Font.paper(17)
    static let bodyBold = Font.paper(17, weight: .semibold)
    static let caption = Font.paper(13, weight: .medium)
    static let wordmark = Font.paper(20, weight: .black)

    /// The SF Symbol for a glyph Rust names.
    static func symbol(_ glyph: String?) -> String? {
        switch glyph {
        case "map": "map"
        case "search": "magnifyingglass"
        case "test": "checklist"
        case "check": "checkmark.seal"
        case "list": "list.bullet"
        case "info": "info.circle"
        case "computer": "desktopcomputer"
        case "add": "plus"
        case "share": "square.and.arrow.up"
        case "ask": "questionmark.bubble"
        case "news": "newspaper"
        case "stop": "stop.fill"
        case "tool": "wrench.and.screwdriver"
        case "credit": "star.circle"
        case "person": "person.fill"
        case "globe": "globe"
        default: nil
        }
    }
}

extension View {
    /// Uppercase condensed titles.
    func gymTitle(_ font: Font = GymStyle.rowTitle, tracking: CGFloat = 0.6) -> some View {
        self.font(font).tracking(tracking).textCase(.uppercase)
    }
}

/// Dims a little while pressed.
struct GymPress: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .opacity(configuration.isPressed ? 0.7 : 1)
            .scaleEffect(configuration.isPressed ? 0.985 : 1)
    }
}

/// The one white-filled button.
struct GymPrimary: View {
    let button: GymButton
    var busy = false
    let tap: (String) -> Void

    var body: some View {
        Button { tap(button.id) } label: {
            HStack(spacing: 10) {
                if busy { ProgressView().tint(.black) }
                if let symbol = GymStyle.symbol(button.glyph) {
                    Image(systemName: symbol).font(.paper(18, weight: .bold))
                }
                Text(button.label).gymTitle(GymStyle.button)
            }
            .foregroundStyle(button.enabled ? Color.black : Color(white: 0.55))
            .frame(maxWidth: .infinity, minHeight: 58)
            .background(RoundedRectangle(cornerRadius: 14)
                .fill(button.enabled ? Color.white : Color(white: 0.20)))
            .shadow(color: .white.opacity(button.enabled ? 0.18 : 0), radius: 18)
        }
        .buttonStyle(GymPress())
        .disabled(!button.enabled)
        .accessibilityIdentifier(button.id)
    }
}

/// An outlined button inside a card or sheet.
struct GymOutlined: View {
    let button: GymButton
    let tap: (String) -> Void

    var body: some View {
        Button { tap(button.id) } label: {
            HStack(spacing: 8) {
                if let symbol = GymStyle.symbol(button.glyph) {
                    Image(systemName: symbol).font(.paper(15, weight: .semibold))
                }
                Text(button.label).font(.paper(16, weight: .semibold)).lineLimit(1)
                    .minimumScaleFactor(0.8)
            }
            .foregroundStyle(.white)
            .padding(.horizontal, 14)
            .frame(minHeight: 44)
            .overlay(RoundedRectangle(cornerRadius: 22).stroke(GymStyle.stroke, lineWidth: 1))
            .contentShape(Rectangle())
        }
        .buttonStyle(GymPress())
        .disabled(!button.enabled)
        .accessibilityIdentifier(button.id)
    }
}

/// A chip: outlined, never filled.
struct GymChip: View {
    let button: GymButton
    let tap: (String) -> Void

    var body: some View {
        Button { tap(button.id) } label: {
            HStack(spacing: 6) {
                if let symbol = GymStyle.symbol(button.glyph) {
                    Image(systemName: symbol).font(.paper(14, weight: .semibold))
                }
                Text(button.label).font(.paper(15, weight: .medium)).lineLimit(2)
                    .multilineTextAlignment(.leading)
            }
            .foregroundStyle(.white)
            .padding(.horizontal, 14)
            .padding(.vertical, 6)
            .frame(minHeight: 40)
            .background(RoundedRectangle(cornerRadius: 20).fill(GymStyle.raised))
            .overlay(RoundedRectangle(cornerRadius: 20).stroke(GymStyle.stroke, lineWidth: 1))
        }
        .buttonStyle(GymPress())
        .disabled(!button.enabled)
        .accessibilityIdentifier(button.id)
    }
}

struct GymLineText: View {
    let line: GymLine

    var body: some View {
        Text(line.text)
            .font(line.tone == "strong" ? GymStyle.bodyBold : GymStyle.body)
            .foregroundStyle(line.tone == "quiet" ? GymStyle.secondary : .white)
            .fixedSize(horizontal: false, vertical: true)
    }
}

struct GymMark: View {
    let mark: String

    var body: some View {
        Group {
            switch mark {
            case "check": Image(systemName: "checkmark").foregroundStyle(.white)
            case "cross": Image(systemName: "xmark").foregroundStyle(Color(white: 0.40))
            case "wait": Image(systemName: "ellipsis").foregroundStyle(GymStyle.secondary)
            case "dot": Image(systemName: "circle.fill").font(.paper(6)).foregroundStyle(.white)
            default: Image(systemName: "minus").foregroundStyle(GymStyle.tertiary)
            }
        }
        .font(.paper(14, weight: .heavy))
        .frame(width: 22)
    }
}

struct GymItemRow: View {
    let item: GymItem

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 6) {
            ForEach(Array(item.marks.enumerated()), id: \.offset) { _, mark in GymMark(mark: mark) }
            VStack(alignment: .leading, spacing: 2) {
                Text(item.text).font(GymStyle.body).foregroundStyle(.white)
                    .fixedSize(horizontal: false, vertical: true)
                if let detail = item.detail {
                    Text(detail).font(.paper(15)).foregroundStyle(GymStyle.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            Spacer(minLength: 4)
            if let trailing = item.trailing {
                Text(trailing).font(GymStyle.bodyBold).foregroundStyle(.white)
            }
        }
    }
}

/// Tests passed without and with the tool, the biggest thing on a result.
struct GymCompareView: View {
    let compare: GymCompare
    var big = false

    var body: some View {
        HStack(alignment: .bottom, spacing: 12) {
            if let without = compare.without {
                VStack(alignment: .leading, spacing: 2) {
                    Text(compare.without_label).font(GymStyle.caption).foregroundStyle(GymStyle.secondary)
                    Text(without).font(big ? GymStyle.hugeNumber : GymStyle.cardNumber)
                        .foregroundStyle(GymStyle.secondary)
                }
                Image(systemName: "arrow.right").font(.paper(20, weight: .bold))
                    .foregroundStyle(GymStyle.secondary).padding(.bottom, 8)
            }
            VStack(alignment: .leading, spacing: 2) {
                Text(compare.with_label).font(GymStyle.caption).foregroundStyle(GymStyle.secondary)
                Text(compare.with).font(big ? GymStyle.hugeNumber : GymStyle.cardNumber)
                    .foregroundStyle(.white)
            }
        }
        .accessibilityElement(children: .combine)
    }
}

/// One side of a run: a block per test.
struct GymBlocks: View {
    let progress: GymProgress

    var body: some View {
        HStack(spacing: 10) {
            HStack(spacing: 4) {
                ForEach(0..<max(progress.total, 1), id: \.self) { i in
                    RoundedRectangle(cornerRadius: 3)
                        .fill(i < progress.done ? Color.white : GymStyle.raised)
                        .overlay(RoundedRectangle(cornerRadius: 3).stroke(GymStyle.stroke, lineWidth: 1))
                        .frame(width: 16, height: 16)
                }
            }
            Text(progress.label).font(GymStyle.body).foregroundStyle(GymStyle.secondary)
                .lineLimit(1).minimumScaleFactor(0.8)
        }
    }
}

/// Wraps its children onto more lines.
struct GymFlow: Layout {
    var spacing: CGFloat = 8

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let width = proposal.width ?? .infinity
        let frames = place(subviews, width: width)
        return CGSize(width: proposal.width ?? (frames.map(\.maxX).max() ?? 0),
                      height: frames.map(\.maxY).max() ?? 0)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        for (subview, frame) in zip(subviews, place(subviews, width: bounds.width)) {
            subview.place(at: CGPoint(x: bounds.minX + frame.minX, y: bounds.minY + frame.minY),
                          proposal: ProposedViewSize(frame.size))
        }
    }

    private func place(_ subviews: Subviews, width: CGFloat) -> [CGRect] {
        var frames: [CGRect] = []
        var x: CGFloat = 0, y: CGFloat = 0, line: CGFloat = 0
        for subview in subviews {
            var size = subview.sizeThatFits(.unspecified)
            if size.width > width {
                size = subview.sizeThatFits(ProposedViewSize(width: width, height: nil))
                size.width = min(size.width, width)
            }
            if x > 0 && x + size.width > width {
                x = 0
                y += line + spacing
                line = 0
            }
            frames.append(CGRect(origin: CGPoint(x: x, y: y), size: size))
            x += size.width + spacing
            line = max(line, size.height)
        }
        return frames
    }
}

// MARK: - Cards

/// A card under a chat reply (`CARD-01` to `CARD-07`), with its chips below.
struct GymCardView: View {
    let card: GymCard
    let tap: (String) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            VStack(alignment: .leading, spacing: 12) {
                header
                if let step = card.step {
                    Text(step).gymTitle(GymStyle.section, tracking: 1.4).foregroundStyle(GymStyle.tertiary)
                }
                if let compare = card.compare { GymCompareView(compare: compare) }
                ForEach(Array(card.progress.enumerated()), id: \.offset) { _, side in GymBlocks(progress: side) }
                ForEach(Array(card.items.enumerated()), id: \.offset) { _, item in GymItemRow(item: item) }
                ForEach(Array(card.lines.enumerated()), id: \.offset) { _, line in GymLineText(line: line) }
                if let primary = card.primary { GymPrimary(button: primary, tap: tap) }
                if !card.secondary.isEmpty {
                    GymFlow { ForEach(card.secondary) { GymOutlined(button: $0, tap: tap) } }
                }
                if let source = card.source {
                    Text(source).font(GymStyle.caption).foregroundStyle(GymStyle.tertiary)
                }
            }
            .padding(16)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(RoundedRectangle(cornerRadius: 16).fill(GymStyle.card))
            .overlay(RoundedRectangle(cornerRadius: 16)
                .stroke(card.busy ? GymStyle.strokeStrong : GymStyle.stroke, lineWidth: card.busy ? 2 : 1))
            if !card.chips.isEmpty {
                GymFlow { ForEach(card.chips) { GymChip(button: $0, tap: tap) } }
            }
        }
        .fixedSize(horizontal: false, vertical: true)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("gym-card-\(card.kind)")
    }

    @ViewBuilder private var header: some View {
        HStack(spacing: 10) {
            if let symbol = GymStyle.symbol(card.icon) {
                Image(systemName: symbol).font(.paper(20, weight: .bold))
            }
            Text(card.title)
                .gymTitle(card.kind == "result" ? GymStyle.cardHeadline : GymStyle.cardTitle)
                .fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 4)
            if card.busy { ProgressView().tint(.white) }
            if let badge = card.badge {
                Text(badge).font(GymStyle.bodyBold)
            }
        }
        .foregroundStyle(.white)
    }
}

/// A chat surface Rust names `gym-card:<id>`: its card from the packet.
struct GymCardSurface: View {
    let resource: String
    @ObservedObject var bridge: MobileBridge

    var body: some View {
        let id = resource.hasPrefix("gym-card:") ? String(resource.dropFirst("gym-card:".count)) : ""
        if let card = bridge.packet?.gymPacket?.cards[id] {
            GymScrollingCard(card: card) { bridge.gym($0) }
        } else {
            EmptyView()
        }
    }
}

/// A card at its own height up to a cap, scrolling inside past it, so a
/// tall card never pushes the chat's header and composer off the screen.
struct GymScrollingCard: View {
    let card: GymCard
    let tap: (String) -> Void
    @State private var height: CGFloat = 0

    var body: some View {
        let cap = UIScreen.main.bounds.height * 0.5
        ScrollView {
            GymCardView(card: card, tap: tap)
                .background(GeometryReader { geo in
                    Color.clear.preference(key: GymCardHeight.self, value: geo.size.height)
                })
        }
        .scrollBounceBehavior(.basedOnSize)
        .frame(height: height == 0 ? nil : min(height, cap))
        .onPreferenceChange(GymCardHeight.self) { height = $0 }
    }
}

private struct GymCardHeight: PreferenceKey {
    static var defaultValue: CGFloat = 0
    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) { value = max(value, nextValue()) }
}

// MARK: - Sheets

/// A sheet over the chat or the menu: `SCR-05`, `SCR-06`, `SCR-11`,
/// `SCR-20`, `SCR-21`, or a stop confirmation.
struct GymSheetView: View {
    let sheet: GymSheet
    let tap: (String) -> Void

    private var footerChoices: Bool { sheet.kind == "publish" || sheet.kind == "stop" }

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text(sheet.title).gymTitle(GymStyle.cardTitle).foregroundStyle(.white)
                Spacer()
                if let close = sheet.close {
                    Button { tap(close.id) } label: {
                        Image(systemName: "xmark").font(.paper(18, weight: .bold))
                            .foregroundStyle(.white).frame(width: 44, height: 44)
                    }
                    .accessibilityLabel(close.label)
                    .accessibilityIdentifier(close.id)
                }
            }
            .padding(.horizontal, 20).padding(.top, 16)
            Divider().overlay(GymStyle.stroke).padding(.horizontal, 20)
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    if let big = sheet.big {
                        Text(big).font(GymStyle.levelNumber).foregroundStyle(.white)
                            .frame(maxWidth: .infinity)
                            .shadow(color: .white.opacity(0.5), radius: 20)
                    }
                    if let headline = sheet.headline {
                        Text(headline).gymTitle(GymStyle.headline).foregroundStyle(.white)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    if let compare = sheet.compare { GymCompareView(compare: compare, big: true) }
                    ForEach(Array(sheet.sections.enumerated()), id: \.offset) { _, section in
                        VStack(alignment: .leading, spacing: 8) {
                            if let heading = section.heading {
                                Text(heading).font(heading.uppercased() == heading ? GymStyle.section : GymStyle.bodyBold)
                                    .tracking(heading.uppercased() == heading ? 1.4 : 0)
                                    .foregroundStyle(heading.uppercased() == heading ? GymStyle.tertiary : .white)
                            }
                            ForEach(Array(section.items.enumerated()), id: \.offset) { _, item in GymItemRow(item: item) }
                            ForEach(Array(section.lines.enumerated()), id: \.offset) { _, line in GymLineText(line: line) }
                        }
                    }
                    // Only Add to the Gym and a stop keep their second
                    // choice beside the primary; other sheets list theirs here.
                    if !footerChoices {
                        VStack(alignment: .leading, spacing: 4) {
                            ForEach(sheet.secondary) { button in GymOutlined(button: button, tap: tap) }
                        }
                    }
                    if let bar = sheet.bar {
                        VStack(alignment: .leading, spacing: 6) {
                            GymXPBar(value: bar.value, max: bar.max)
                            Text(bar.label).font(GymStyle.caption).foregroundStyle(GymStyle.secondary)
                        }
                    }
                }
                .padding(20)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            VStack(spacing: 10) {
                if let next = sheet.next {
                    Text(next).font(GymStyle.body).foregroundStyle(GymStyle.secondary)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                if let primary = sheet.primary { GymPrimary(button: primary, busy: sheet.busy, tap: tap) }
                ForEach(footerChoices ? sheet.secondary : []) { button in
                    Button { tap(button.id) } label: {
                        HStack(spacing: 6) {
                            if let symbol = GymStyle.symbol(button.glyph) { Image(systemName: symbol) }
                            Text(button.label)
                        }
                        .font(GymStyle.body).foregroundStyle(GymStyle.secondary)
                        .frame(maxWidth: .infinity, minHeight: 44)
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(GymPress())
                    .accessibilityIdentifier(button.id)
                }
            }
            .padding(.horizontal, 20).padding(.bottom, 16)
        }
        .background(Color(white: 0.04).ignoresSafeArea())
        .preferredColorScheme(.dark)
    }
}

/// A level bar.
struct GymXPBar: View {
    let value: Int
    let max: Int

    var body: some View {
        GeometryReader { geo in
            ZStack(alignment: .leading) {
                Capsule().fill(GymStyle.raised)
                Capsule().stroke(GymStyle.stroke, lineWidth: 1)
                Capsule().fill(Color.white)
                    .frame(width: Swift.max(8, geo.size.width * Swift.min(1, Double(value) / Double(Swift.max(max, 1)))))
                    .shadow(color: .white.opacity(0.35), radius: 6)
            }
        }
        .frame(height: 8)
    }
}

// MARK: - Menu and first run

/// The white power symbol.
struct GymPowerSymbol: View {
    var body: some View {
        GeometryReader { geo in
            let s = Swift.min(geo.size.width, geo.size.height)
            let w = s * 0.13
            ZStack {
                Circle().trim(from: 0.10, to: 0.90)
                    .stroke(style: StrokeStyle(lineWidth: w, lineCap: .round))
                    .rotationEffect(.degrees(-90))
                    .padding(w / 2 + s * 0.06)
                Capsule().frame(width: w, height: s * 0.46).offset(y: -s * 0.22)
            }
            .frame(width: s, height: s)
        }
        .aspectRatio(1, contentMode: .fit)
    }
}

/// The Grid floor, the Gym building, and Coder, drawn in code.
struct GymHero: View {
    let status: String

    var body: some View {
        GeometryReader { geo in
            let w = geo.size.width, h = geo.size.height
            ZStack {
                LinearGradient(colors: [Color(white: 0.10), .black], startPoint: .top, endPoint: .bottom)
                Canvas { ctx, size in
                    let hy = size.height * 0.42
                    let vp = CGPoint(x: size.width / 2, y: hy)
                    var path = Path()
                    for i in -22...22 {
                        let x = size.width / 2 + CGFloat(i) * size.width / 22 * 1.6
                        path.move(to: CGPoint(x: x, y: size.height))
                        path.addLine(to: vp)
                    }
                    for i in 1...18 {
                        let t = CGFloat(i) / 18
                        let y = hy + (size.height - hy) * t * t
                        path.move(to: CGPoint(x: 0, y: y))
                        path.addLine(to: CGPoint(x: size.width, y: y))
                    }
                    ctx.stroke(path, with: .color(Color(white: 1, opacity: 0.22)), lineWidth: 0.8)
                }
                .mask(LinearGradient(stops: [.init(color: .clear, location: 0.40),
                                             .init(color: .white.opacity(0.5), location: 0.54),
                                             .init(color: .white, location: 1)],
                                     startPoint: .top, endPoint: .bottom))
                // The Gym.
                Path { p in
                    let gw = w * 0.3, gh = h * 0.28, gx = w * 0.51, gy = h * 0.2
                    p.addRect(CGRect(x: gx, y: gy + gh * 0.28, width: gw, height: gh * 0.72))
                    p.move(to: CGPoint(x: gx - gw * 0.04, y: gy + gh * 0.28))
                    p.addLine(to: CGPoint(x: gx + gw * 0.5, y: gy))
                    p.addLine(to: CGPoint(x: gx + gw * 1.04, y: gy + gh * 0.28))
                    for i in 1..<6 {
                        let x = gx + gw * CGFloat(i) / 6
                        p.move(to: CGPoint(x: x, y: gy + gh * 0.34))
                        p.addLine(to: CGPoint(x: x, y: gy + gh))
                    }
                }
                .stroke(Color.white.opacity(0.75), lineWidth: 1.2)
                Text("GYM").font(.paper(9, weight: .black)).tracking(2)
                    .foregroundStyle(.white).position(x: w * 0.66, y: h * 0.26)
                Rectangle().fill(LinearGradient(colors: [.white, .white.opacity(0.3)], startPoint: .top, endPoint: .bottom))
                    .frame(width: w * 0.06, height: h * 0.12).position(x: w * 0.66, y: h * 0.42)
                    .shadow(color: .white.opacity(0.9), radius: 14)
                // Coder.
                ZStack {
                    RoundedRectangle(cornerRadius: 14).frame(width: w * 0.16, height: h * 0.34)
                    RoundedRectangle(cornerRadius: 10).frame(width: w * 0.09, height: h * 0.13).offset(y: -h * 0.24)
                    Capsule().fill(.white.opacity(0.85)).frame(width: w * 0.055, height: 3).offset(y: -h * 0.245)
                    GymPowerSymbol().foregroundStyle(.white).frame(width: w * 0.055).offset(y: -h * 0.03)
                        .shadow(color: .white, radius: 10)
                }
                .foregroundStyle(LinearGradient(colors: [Color(white: 0.16), Color(white: 0.03)],
                                                startPoint: .topLeading, endPoint: .bottomTrailing))
                .position(x: w * 0.24, y: h * 0.52)
                // The status pill.
                HStack(spacing: 8) {
                    Circle().fill(.white).frame(width: 8, height: 8)
                    Text(status).font(GymStyle.caption).foregroundStyle(.white)
                }
                .padding(.horizontal, 12).padding(.vertical, 6)
                .background(Capsule().fill(Color.black.opacity(0.7)))
                .overlay(Capsule().stroke(GymStyle.stroke, lineWidth: 1))
                .position(x: w / 2, y: h - 22)
            }
        }
        .clipShape(RoundedRectangle(cornerRadius: 16))
        .overlay(RoundedRectangle(cornerRadius: 16).stroke(GymStyle.stroke, lineWidth: 1))
        .accessibilityHidden(true)
    }
}

/// `SCR-01` Main menu.
struct GymMenuView: View {
    let menu: GymMenu
    let tap: (String) -> Void

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                HStack(spacing: 10) {
                    GymPowerSymbol().frame(width: 26, height: 26).shadow(color: .white.opacity(0.75), radius: 14)
                    Text("OPENAGENTS").font(GymStyle.wordmark).tracking(3)
                }
                .foregroundStyle(.white)
                .accessibilityLabel("OpenAgents")
                player
                GymHero(status: menu.status).frame(height: 230)
                (Text("Next: ").font(GymStyle.bodyBold).foregroundStyle(.white)
                    + Text(menu.next.hasPrefix("Next: ") ? String(menu.next.dropFirst(6)) : menu.next)
                        .font(GymStyle.body).foregroundStyle(GymStyle.secondary))
                    .fixedSize(horizontal: false, vertical: true)
                    .accessibilityIdentifier("menu-next")
                Button { tap(menu.primary.id) } label: {
                    HStack(spacing: 12) {
                        Image(systemName: "bubble.left.fill").font(.paper(22, weight: .bold)).frame(width: 40)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(menu.primary.label).gymTitle().lineLimit(1).minimumScaleFactor(0.8)
                            Text(menu.primary_subtitle).font(.paper(16)).opacity(0.62)
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                        Image(systemName: "chevron.right").font(.paper(17, weight: .bold)).opacity(0.62)
                    }
                    .foregroundStyle(.black)
                    .padding(12)
                    .frame(maxWidth: .infinity, minHeight: 76)
                    .background(RoundedRectangle(cornerRadius: 14).fill(Color.white))
                    .shadow(color: .white.opacity(0.18), radius: 18)
                }
                .buttonStyle(GymPress())
                .accessibilityIdentifier(menu.primary.id)
                GymFlow { ForEach(menu.chips) { GymChip(button: $0, tap: tap) } }
                ForEach(menu.rows, id: \.button.id) { row in
                    Button { tap(row.button.id) } label: {
                        HStack(spacing: 12) {
                            Image(systemName: GymStyle.symbol(row.glyph) ?? "circle")
                                .font(.paper(22, weight: .bold)).frame(width: 40)
                            VStack(alignment: .leading, spacing: 2) {
                                Text(row.button.label).gymTitle().lineLimit(1).minimumScaleFactor(0.8)
                                Text(row.subtitle).font(.paper(16)).foregroundStyle(GymStyle.secondary)
                            }
                            .frame(maxWidth: .infinity, alignment: .leading)
                            Image(systemName: "chevron.right").font(.paper(17, weight: .bold))
                                .foregroundStyle(GymStyle.secondary)
                        }
                        .foregroundStyle(.white)
                        .padding(12)
                        .frame(maxWidth: .infinity, minHeight: 76)
                        .background(RoundedRectangle(cornerRadius: 14).fill(GymStyle.surface))
                        .overlay(RoundedRectangle(cornerRadius: 14).stroke(GymStyle.stroke, lineWidth: 1))
                    }
                    .buttonStyle(GymPress())
                    .accessibilityIdentifier(row.button.id)
                }
                HStack {
                    Circle().fill(.white).frame(width: 7, height: 7)
                    Text(menu.footer).font(GymStyle.caption).foregroundStyle(GymStyle.tertiary)
                }
                .padding(.top, 4)
            }
            .padding(.horizontal, 20)
            .padding(.vertical, 12)
        }
        .background(Color.black.ignoresSafeArea())
    }

    private var player: some View {
        HStack(spacing: 12) {
            ZStack {
                RoundedRectangle(cornerRadius: 12)
                    .fill(LinearGradient(colors: [Color(white: 0.22), Color(white: 0.06)],
                                         startPoint: .topLeading, endPoint: .bottomTrailing))
                RoundedRectangle(cornerRadius: 12).stroke(GymStyle.strokeStrong, lineWidth: 1)
                GymPowerSymbol().frame(width: 24).foregroundStyle(.white).shadow(color: .white.opacity(0.6), radius: 6)
            }
            .frame(width: 52, height: 52)
            VStack(alignment: .leading, spacing: 6) {
                Text(menu.player.name).font(GymStyle.bodyBold).foregroundStyle(.white)
                HStack(spacing: 8) {
                    if let level = menu.player.level {
                        Text("LEVEL \(level)").gymTitle(GymStyle.section, tracking: 1).foregroundStyle(.white)
                        GymXPBar(value: menu.player.bar_value, max: menu.player.bar_max)
                    } else {
                        // Still reading: a gray bar, never a 0.
                        RoundedRectangle(cornerRadius: 4).fill(GymStyle.raised).frame(width: 140, height: 8)
                    }
                }
                if let xp = menu.player.xp_label {
                    Text(xp).font(GymStyle.caption).foregroundStyle(GymStyle.secondary)
                } else {
                    RoundedRectangle(cornerRadius: 4).fill(GymStyle.raised).frame(width: 70, height: 10)
                }
            }
        }
        .padding(12)
        .background(RoundedRectangle(cornerRadius: 16).fill(GymStyle.surface))
        .overlay(RoundedRectangle(cornerRadius: 16).stroke(GymStyle.stroke, lineWidth: 1))
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier("menu-player")
    }
}

/// `SCR-02` Choose your agent, and the intro's end card.
struct GymFirstRunView: View {
    let first: GymFirstRun
    let tap: (String) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            if let indicator = first.indicator {
                HStack {
                    Text(indicator).gymTitle(GymStyle.section, tracking: 1.4).foregroundStyle(GymStyle.secondary)
                    Spacer()
                    HStack(spacing: 6) {
                        ForEach(1...3, id: \.self) { dot in
                            Circle().fill(dot <= first.dot ? Color.white : GymStyle.raised)
                                .overlay(Circle().stroke(GymStyle.stroke, lineWidth: 1))
                                .frame(width: 9, height: 9)
                        }
                    }
                }
            }
            Spacer(minLength: 0)
            Text(first.title).font(.paper(26, weight: .bold)).foregroundStyle(.white)
            ForEach(first.lines, id: \.self) { line in
                Text(line).font(GymStyle.body).foregroundStyle(GymStyle.secondary)
            }
            if let agent = first.agent, agent.count == 2 {
                HStack(spacing: 16) {
                    ZStack {
                        RoundedRectangle(cornerRadius: 18).fill(LinearGradient(colors: [Color(white: 0.16), Color(white: 0.03)],
                                                                               startPoint: .topLeading, endPoint: .bottomTrailing))
                        GymPowerSymbol().frame(width: 36).foregroundStyle(.white).shadow(color: .white, radius: 12)
                    }
                    .frame(width: 88, height: 120)
                    VStack(alignment: .leading, spacing: 6) {
                        Text(agent[0]).gymTitle().foregroundStyle(.white)
                        Text(agent[1]).font(GymStyle.body).foregroundStyle(GymStyle.secondary)
                    }
                    Spacer()
                    Image(systemName: "checkmark.circle.fill").font(.paper(24)).foregroundStyle(.white)
                }
                .padding(16)
                .background(RoundedRectangle(cornerRadius: 16).fill(GymStyle.card))
                .overlay(RoundedRectangle(cornerRadius: 16).stroke(GymStyle.strokeStrong, lineWidth: 2))
                .accessibilityElement(children: .combine)
            } else {
                GymHero(status: "GYM OPEN").frame(height: 260)
            }
            Spacer(minLength: 0)
            Text(first.next).font(GymStyle.body).foregroundStyle(GymStyle.secondary)
            GymPrimary(button: first.primary, tap: tap)
            if let secondary = first.secondary {
                Button { tap(secondary.id) } label: {
                    Text(secondary.label).font(GymStyle.body).foregroundStyle(GymStyle.secondary)
                        .frame(maxWidth: .infinity, minHeight: 44)
                }
                .accessibilityIdentifier(secondary.id)
            }
        }
        .padding(20)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(Color.black.ignoresSafeArea())
    }
}
