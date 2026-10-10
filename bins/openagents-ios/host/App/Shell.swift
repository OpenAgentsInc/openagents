// The phone's shell (#11126): the top bar (menu, the Coder / Verse switch,
// new chat), the feature cards on a new chat, and the drawer with the main
// places and recent chats. Rust owns the state (`coder_tab::shell`); this
// file draws it and sends the person's taps back as shell actions.
import SwiftUI
import UIKit

/// Rust's `coder_tab::ShellView`.
struct ShellState: Decodable, Equatable {
    struct Card: Decodable, Equatable, Identifiable {
        let id: String
        let title: String
        let line: String
    }
    struct Recent: Decodable, Equatable, Identifiable {
        let index: Int
        let title: String
        let detail: String
        var id: Int { index }
    }
    struct Drawer: Decodable, Equatable {
        let query: String
        let recent: [Recent]
        let more: Bool
    }
    struct Carousel: Decodable, Equatable {
        /// The card the carousel opens on, by index.
        let start: Int
        let dwell_ms: Int
        let resume_ms: Int
    }
    /// `coder` (the conversation) or `verse` (the Grid world).
    let place: String
    let carousel: Carousel?
    /// `new`, `chat`, or `list`.
    let screen: String
    let cards: [Card]
    let drawer: Drawer?
}

/// A round button in the top bar.
struct ShellCircleButton: View {
    @Environment(\.appColors) private var appColors
    let symbol: String
    let label: String
    let identifier: String
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 17, weight: .medium))
                .foregroundStyle(appColors.primary)
                .frame(width: 44, height: 44)
                .background(Circle().fill(appColors.raised))
                .overlay(Circle().strokeBorder(appColors.border, lineWidth: 0.5))
                .contentShape(Circle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(label)
        .accessibilityIdentifier(identifier)
    }
}

/// The menu button every place starts with: a tap opens the drawer, and a
/// long press reports a problem with the screen on view.
struct ShellMenuButton: View {
    let open: () -> Void
    var report: (() -> Void)?

    var body: some View {
        ShellCircleButton(symbol: "line.3.horizontal", label: "Open menu", identifier: "shell-menu", action: open)
            .simultaneousGesture(LongPressGesture(minimumDuration: 0.6).onEnded { _ in report?() })
    }
}

/// The top bar over the chat and the Verse: the menu, the Coder / Verse
/// switch on a new chat and in the Verse, and a new chat once a
/// conversation is open.
struct ShellTopBar: View {
    @Environment(\.appColors) private var appColors
    let state: ShellState?
    let bridge: MobileBridge
    let openDrawer: () -> Void
    let report: () -> Void

    var body: some View {
        ZStack {
            if state?.screen == "new" || state?.place == "verse" {
                ShellPlaceSwitch(verse: state?.place == "verse") { verse in
                    bridge.shell("switch", ["verse": verse])
                }
            }
            HStack {
                ShellMenuButton(open: openDrawer, report: report)
                Spacer()
                if state?.screen == "chat" && state?.place != "verse" {
                    ShellCircleButton(symbol: "square.and.pencil", label: "New chat",
                                      identifier: "shell-new-chat") { bridge.shell("new_chat") }
                }
            }
        }
        .padding(.horizontal, 16)
        .padding(.top, 4)
        .padding(.bottom, 6)
        .frame(maxWidth: .infinity)
        .background(appColors.background.opacity(0.001))
    }
}

/// Coder or Verse: one capsule, the chosen half filled.
struct ShellPlaceSwitch: View {
    @Environment(\.appColors) private var appColors
    @Namespace private var fill
    let verse: Bool
    let choose: (Bool) -> Void

    var body: some View {
        HStack(spacing: 0) {
            segment("Coder", chosen: !verse) { choose(false) }
            segment("Verse", chosen: verse) { choose(true) }
        }
        .padding(4)
        .background(Capsule().fill(appColors.raised))
        .overlay(Capsule().strokeBorder(appColors.border, lineWidth: 0.5))
        .animation(.snappy(duration: 0.25), value: verse)
    }

    private func segment(_ title: String, chosen: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(title)
                .font(.paper(16, weight: chosen ? .semibold : .regular))
                .foregroundStyle(chosen ? appColors.primary : appColors.secondary)
                .padding(.horizontal, 18)
                .frame(height: 38)
                .background {
                    if chosen {
                        Capsule().fill(appColors.primary.opacity(0.12))
                            .matchedGeometryEffect(id: "chosen", in: fill)
                    }
                }
                .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(chosen ? .isSelected : [])
        .accessibilityIdentifier("shell-switch-\(title.lowercased())")
    }
}

/// When the feature cards move on by themselves: the same rules as Rust's
/// `carousel::AutoScroll` (one card each dwell, nothing while a finger is
/// down, a pause after it lifts, never with Reduce Motion).
struct CarouselClock {
    var dwell: TimeInterval
    var resume: TimeInterval
    var reduceMotion: Bool
    var touching = false
    var since: TimeInterval

    mutating func touch() { touching = true }

    mutating func release(_ now: TimeInterval) {
        touching = false
        since = now + resume
    }

    mutating func step(_ now: TimeInterval) -> Bool {
        if reduceMotion || touching || now < since + dwell { return false }
        since = now
        return true
    }
}

/// The feature cards on a new chat: a fan of tilted cards to swipe
/// through (the row repeats, so there are cards on both sides), the chosen
/// card's headline and line, and **Try it**. It opens on the card Rust
/// picks and moves on by itself (`CarouselClock`).
struct HomeCardsSurface: View {
    @Environment(\.appColors) private var appColors
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    let cards: [ShellState.Card]
    var carousel: ShellState.Carousel?
    let tryIt: (String) -> Void
    @State private var chosen: String?
    @State private var clock: CarouselClock?
    /// The keyboard is up: the cards step aside so the composer stays in
    /// view above it.
    @State private var keyboard = false

    /// How many times the row repeats; the middle copy shows first.
    private static let copies = 7
    private static let side: CGFloat = 96

    private struct Slot: Identifiable {
        let id: String
        let card: ShellState.Card
    }

    private var slots: [Slot] {
        (0..<Self.copies).flatMap { copy in
            cards.map { Slot(id: "\(copy)#\($0.id)", card: $0) }
        }
    }

    private var current: ShellState.Card? {
        slots.first { $0.id == chosen }?.card ?? cards.first
    }

    /// One card on, smoothly; near the end of the copies it first jumps
    /// back to the middle copy of the same card, which looks the same.
    private func advance(_ reader: ScrollViewProxy) {
        let all = slots
        guard !all.isEmpty else { return }
        var at = all.firstIndex { $0.id == chosen } ?? (Self.copies / 2) * cards.count
        if at >= all.count - cards.count {
            at = (Self.copies / 2) * cards.count + at % cards.count
            reader.scrollTo(all[at].id, anchor: .center)
        }
        let next = all[at + 1].id
        withAnimation(.easeInOut(duration: 0.8)) {
            reader.scrollTo(next, anchor: .center)
            chosen = next
        }
    }

    var body: some View {
        Group {
            if keyboard {
                Color.clear.frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                cardsBody
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: UIResponder.keyboardWillShowNotification)) { _ in
            keyboard = true
        }
        .onReceive(NotificationCenter.default.publisher(for: UIResponder.keyboardWillHideNotification)) { _ in
            keyboard = false
        }
    }

    private var cardsBody: some View {
        VStack(spacing: 16) {
            Spacer(minLength: 0)
            GeometryReader { outer in
                ScrollViewReader { reader in
                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack(spacing: 22) {
                            ForEach(slots) { slot in
                                HomeCardArt(id: slot.card.id)
                                    .frame(width: Self.side, height: Self.side)
                                    .visualEffect { content, proxy in
                                        let middle = outer.size.width / 2
                                        let offset = proxy.frame(in: .named("home-cards")).midX - middle
                                        let turn = max(-2, min(2, offset / 118))
                                        return content
                                            .rotationEffect(.degrees(turn * 12))
                                            .offset(y: turn * turn * 16)
                                            .scaleEffect(1 - min(abs(turn), 1.5) * 0.06)
                                    }
                                    .id(slot.id)
                                    .onTapGesture {
                                        withAnimation(.snappy) { chosen = slot.id }
                                        clock?.release(Date().timeIntervalSinceReferenceDate)
                                    }
                                    .accessibilityElement(children: .ignore)
                                    .accessibilityLabel(slot.card.title)
                                    .accessibilityAddTraits(.isButton)
                            }
                        }
                        .scrollTargetLayout()
                        .padding(.vertical, 50)
                    }
                    .coordinateSpace(name: "home-cards")
                    .scrollClipDisabled()
                    .contentMargins(.horizontal, max(0, (outer.size.width - Self.side) / 2), for: .scrollContent)
                    .scrollTargetBehavior(.viewAligned)
                    .scrollPosition(id: $chosen, anchor: .center)
                    // A finger on the cards stops them; they move again a
                    // while after it lifts.
                    .simultaneousGesture(DragGesture(minimumDistance: 0)
                        .onChanged { _ in clock?.touch() }
                        .onEnded { _ in clock?.release(Date().timeIntervalSinceReferenceDate) })
                    .onAppear {
                        guard chosen == nil, !cards.isEmpty else { return }
                        let index = min(max(carousel?.start ?? 0, 0), cards.count - 1)
                        let start = "\(Self.copies / 2)#\(cards[index].id)"
                        clock = CarouselClock(dwell: Double(carousel?.dwell_ms ?? 3500) / 1000,
                                              resume: Double(carousel?.resume_ms ?? 4000) / 1000,
                                              reduceMotion: reduceMotion,
                                              since: Date().timeIntervalSinceReferenceDate)
                        DispatchQueue.main.async {
                            reader.scrollTo(start, anchor: .center)
                            chosen = start
                        }
                    }
                    .onChange(of: reduceMotion) { _, on in clock?.reduceMotion = on }
                    .task {
                        while !Task.isCancelled {
                            try? await Task.sleep(for: .milliseconds(250))
                            guard clock?.step(Date().timeIntervalSinceReferenceDate) == true else { continue }
                            advance(reader)
                        }
                    }
                }
            }
            .frame(height: Self.side + 120)
            if let card = current {
                VStack(spacing: 6) {
                    Text(card.title)
                        .font(.paper(19, weight: .semibold))
                        .foregroundStyle(appColors.primary)
                    Text(card.line)
                        .font(.paper(15))
                        .foregroundStyle(appColors.secondary)
                        .multilineTextAlignment(.center)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .padding(.horizontal, 32)
                .id(card.id)
                .transition(.opacity)
                Button { tryIt(card.id) } label: {
                    Text("Try it")
                        .font(.paper(16, weight: .semibold))
                        .foregroundStyle(appColors.primary)
                        .padding(.horizontal, 22)
                        .frame(height: 44)
                        .background(Capsule().fill(appColors.raised))
                        .overlay(Capsule().strokeBorder(appColors.border, lineWidth: 0.5))
                }
                .buttonStyle(.plain)
                .padding(.top, 6)
                .accessibilityIdentifier("shell-try-\(card.id)")
            }
            Spacer(minLength: 0)
            Spacer(minLength: 0)
        }
        .animation(.snappy(duration: 0.2), value: current?.id)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

/// One card's picture: a soft gradient tile with the card's glyph.
struct HomeCardArt: View {
    let id: String

    private var look: (symbol: String, colors: [Color]) {
        switch id {
        case "verse": ("globe.americas.fill", [Color(red: 0.16, green: 0.52, blue: 0.86),
                                              Color(red: 0.36, green: 0.80, blue: 0.86)])
        case "coder": ("chevron.left.forwardslash.chevron.right", [Color(red: 0.42, green: 0.32, blue: 0.86),
                                                                   Color(red: 0.72, green: 0.56, blue: 0.98)])
        case "codebase": ("folder.fill", [Color(red: 0.86, green: 0.42, blue: 0.24),
                                          Color(red: 0.98, green: 0.70, blue: 0.40)])
        case "roadmap": ("map.fill", [Color(red: 0.12, green: 0.56, blue: 0.40),
                                      Color(red: 0.46, green: 0.82, blue: 0.56)])
        default: ("sparkles", [Color.gray, Color.gray.opacity(0.6)])
        }
    }

    var body: some View {
        RoundedRectangle(cornerRadius: 20, style: .continuous)
            .fill(LinearGradient(colors: look.colors, startPoint: .topLeading, endPoint: .bottomTrailing))
            .overlay {
                Image(systemName: look.symbol)
                    .font(.system(size: 36, weight: .semibold))
                    .foregroundStyle(.white.opacity(0.92))
                    .shadow(color: .black.opacity(0.15), radius: 6, y: 3)
            }
            .shadow(color: .black.opacity(0.25), radius: 12, y: 8)
    }
}

/// A place the drawer opens.
struct ShellPlace: Identifiable {
    let id: String
    let title: String
    let symbol: String
}

/// The drawer: the app's name and search, the main places, recent chats
/// with See all, a new chat, and the account.
struct ShellDrawer: View {
    @Environment(\.appColors) private var appColors
    let state: ShellState?
    let bridge: MobileBridge
    let go: (String) -> Void
    let close: () -> Void
    @State private var searching = false
    @State private var query = ""
    @FocusState private var searchFocused: Bool

    private var places: [ShellPlace] {
        var places = [
            ShellPlace(id: "code", title: "Coder", symbol: "chevron.left.forwardslash.chevron.right"),
            ShellPlace(id: "computers", title: "Computers", symbol: "desktopcomputer"),
            ShellPlace(id: "wallet", title: "Wallet", symbol: "bitcoinsign.circle"),
            ShellPlace(id: "verse", title: "Verse", symbol: "globe"),
        ]
        places.append(ShellPlace(id: "settings", title: "Settings", symbol: "gearshape"))
        return places
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header
            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    if !searching {
                        ForEach(places) { place in
                            row(place.title, symbol: place.symbol, identifier: "shell-place-\(place.id)") {
                                go(place.id)
                            }
                        }
                        // Running and the account's chats, once signed in
                        // (#11107, #11165).
                        LinkDrawerSection(bridge: bridge, go: go)
                        Divider().overlay(appColors.border).padding(.vertical, 14)
                    }
                    recent
                }
                .padding(.horizontal, 24)
            }
            .scrollDismissesKeyboard(.interactively)
            footer
        }
        .background(appColors.background.ignoresSafeArea())
    }

    private var header: some View {
        HStack(spacing: 12) {
            if searching {
                HStack(spacing: 8) {
                    Image(systemName: "magnifyingglass").foregroundStyle(appColors.secondary)
                    TextField("Search chats", text: $query)
                        .font(.paper(16))
                        .focused($searchFocused)
                        .submitLabel(.search)
                        .autocorrectionDisabled()
                        .accessibilityIdentifier("shell-search-field")
                }
                .padding(.horizontal, 14)
                .frame(height: 44)
                .background(Capsule().fill(appColors.raised))
                Button("Cancel") {
                    searching = false
                    query = ""
                    bridge.shell("search", ["query": ""])
                }
                .font(.paper(15))
                .foregroundStyle(appColors.primary)
                .accessibilityIdentifier("shell-search-cancel")
            } else {
                Text("OpenAgents")
                    .font(.paper(24, weight: .bold))
                    .foregroundStyle(appColors.primary)
                Spacer()
                ShellCircleButton(symbol: "magnifyingglass", label: "Search chats", identifier: "shell-search") {
                    searching = true
                    searchFocused = true
                }
            }
        }
        .padding(.horizontal, 24)
        .padding(.top, 8)
        .padding(.bottom, 12)
        .onChange(of: query) { _, query in bridge.shell("search", ["query": query]) }
    }

    @ViewBuilder private var recent: some View {
        let drawer = state?.drawer
        let rows = drawer?.recent ?? []
        if rows.isEmpty {
            Text(searching && !query.isEmpty ? "No chats match." : "No chats yet.")
                .font(.paper(15))
                .foregroundStyle(appColors.secondary)
                .padding(.vertical, 12)
        }
        ForEach(rows) { chat in
            Button {
                bridge.shell("open", ["index": chat.index])
                go("chat")
            } label: {
                Text(chat.title.isEmpty ? "New chat" : chat.title)
                    .font(.paper(17))
                    .foregroundStyle(appColors.primary)
                    .lineLimit(1)
                    .frame(maxWidth: .infinity, minHeight: 50, alignment: .leading)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityHint(chat.detail)
            .accessibilityIdentifier("shell-recent-\(chat.index)")
        }
        if drawer?.more == true {
            Button {
                bridge.shell("see_all")
                go("chat")
            } label: {
                Text("See all…")
                    .font(.paper(17))
                    .foregroundStyle(appColors.secondary)
                    .frame(maxWidth: .infinity, minHeight: 50, alignment: .leading)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier("shell-see-all")
        }
    }

    private var footer: some View {
        HStack {
            Button {
                bridge.shell("new_chat")
                go("chat")
            } label: {
                Label("Chat", systemImage: "square.and.pencil")
                    .font(.paper(17, weight: .semibold))
                    .foregroundStyle(appColors.background)
                    .padding(.horizontal, 22)
                    .frame(height: 52)
                    .background(Capsule().fill(appColors.primary))
            }
            .buttonStyle(.plain)
            .accessibilityLabel("New chat")
            .accessibilityIdentifier("shell-chat")
            Spacer()
            Button { go("settings") } label: {
                Image(systemName: "person.fill")
                    .font(.system(size: 20, weight: .medium))
                    .foregroundStyle(appColors.primary)
                    .frame(width: 52, height: 52)
                    .background(Circle().fill(appColors.raised))
                    .overlay(Circle().strokeBorder(appColors.border, lineWidth: 0.5))
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Account")
            .accessibilityIdentifier("shell-account")
        }
        .padding(.horizontal, 24)
        .padding(.bottom, 8)
    }

    private func row(_ title: String, symbol: String, identifier: String,
                     action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack(spacing: 16) {
                Image(systemName: symbol)
                    .font(.system(size: 19, weight: .regular))
                    .frame(width: 26)
                Text(title).font(.paper(17))
                Spacer()
            }
            .foregroundStyle(appColors.primary)
            .frame(minHeight: 50)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier(identifier)
    }
}

/// Rust's `links::Card`: a link a reply contains, as its card shows it.
struct LinkCard: Decodable, Equatable {
    let url: String
    let title: String
    let site: String
    /// The card has a picture, read as its surface's image.
    let image: Bool
}

/// A link card under a reply (#11126): the page's picture when it has one,
/// then its title and site. A tap opens the page in the browser.
struct LinkCardSurface: View {
    @Environment(\.appColors) private var appColors
    let resource: String
    let label: String
    let card: LinkCard?
    @ObservedObject var bridge: MobileBridge
    @State private var picture: UIImage?

    private var pictured: Bool { card?.image == true }

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: 14, style: .continuous)
        Button(action: open) {
            VStack(alignment: .leading, spacing: 0) {
                if pictured {
                    // The picture fills its band and is cut to it.
                    appColors.border.opacity(0.4)
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                        .overlay {
                            if let picture {
                                Image(uiImage: picture).resizable().scaledToFill()
                            }
                        }
                        .clipped()
                }
                VStack(alignment: .leading, spacing: 3) {
                    Text(card?.title ?? label)
                        .font(.paper(15, weight: .semibold))
                        .foregroundStyle(appColors.primary)
                        .lineLimit(pictured ? 1 : 2)
                        .multilineTextAlignment(.leading)
                    HStack(spacing: 5) {
                        Image(systemName: "link").font(.system(size: 11, weight: .semibold))
                        Text(card?.site ?? "").font(.paper(13)).lineLimit(1)
                    }
                    .foregroundStyle(appColors.secondary)
                }
                .padding(.horizontal, 14)
                .padding(.vertical, 12)
                .frame(maxWidth: .infinity, maxHeight: pictured ? nil : .infinity, alignment: .leading)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .background(shape.fill(appColors.raised))
            .clipShape(shape)
            .overlay(shape.strokeBorder(appColors.border, lineWidth: 0.5))
            .contentShape(shape)
        }
        .buttonStyle(.plain)
        .disabled(link == nil)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(spoken)
        .accessibilityAddTraits(.isLink)
        .accessibilityIdentifier(resource)
        .task(id: "\(resource)#\(pictured)") {
            guard pictured else { return }
            bridge.image(resource) { picture = $0 }
        }
    }

    private var spoken: String {
        guard let card else { return label }
        return card.title == card.site ? card.site : "\(card.title), \(card.site)"
    }

    /// Only an https link the reply contains opens.
    private var link: URL? {
        guard let card, let url = URL(string: card.url), url.scheme == "https" else { return nil }
        return url
    }

    private func open() {
        if let link { UIApplication.shared.open(link) }
    }
}

/// A reply's **Enter the Grid** card (Rust's `verse-portal` surface), when
/// the answer offered the Verse: a tap switches to the Grid.
struct VersePortalSurface: View {
    @Environment(\.appColors) private var appColors
    let label: String
    let enter: () -> Void

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: 14, style: .continuous)
        Button(action: enter) {
            HStack(spacing: 14) {
                HomeCardArt(id: "verse")
                    .frame(width: 64, height: 64)
                VStack(alignment: .leading, spacing: 4) {
                    Text(label)
                        .font(.paper(16, weight: .semibold))
                        .foregroundStyle(appColors.primary)
                    Text("The Verse")
                        .font(.paper(13))
                        .foregroundStyle(appColors.secondary)
                }
                Spacer(minLength: 0)
                Image(systemName: "arrow.right.circle.fill")
                    .font(.system(size: 26, weight: .semibold))
                    .foregroundStyle(appColors.primary)
            }
            .padding(.horizontal, 16)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
            .background(shape.fill(appColors.raised))
            .overlay(shape.strokeBorder(appColors.border, lineWidth: 0.5))
            .contentShape(shape)
        }
        .buttonStyle(.plain)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(label)
        .accessibilityAddTraits(.isButton)
        .accessibilityIdentifier("verse-portal")
    }
}
