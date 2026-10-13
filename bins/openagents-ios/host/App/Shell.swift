// The phone's shell (#11126): the top bar (menu, the Coder / Verse switch,
// new chat) and the drawer with the main places and recent chats. Rust owns
// the state (`coder_tab::shell`); this file draws it and sends the person's
// taps back as shell actions.
import SwiftUI
import UIKit

/// Rust's `coder_tab::ShellView`.
struct ShellState: Decodable, Equatable {
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
    /// `coder` (the conversation) or `verse` (the Grid world).
    let place: String
    /// `new`, `chat`, or `list`.
    let screen: String
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

/// The Verse's picture on a reply's **Enter the Grid** card: a Coder Noir
/// tile (the theme's raised gray, a hairline edge, no hue) with the globe in
/// the primary ink.
struct VerseTileArt: View {
    @Environment(\.appColors) private var appColors
    private let symbol = "globe.americas.fill"

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: 20, style: .continuous)
        shape
            .fill(appColors.raised)
            .overlay(shape.strokeBorder(appColors.border, lineWidth: 1))
            .overlay {
                Image(systemName: symbol)
                    .font(.system(size: 34, weight: .semibold))
                    .foregroundStyle(appColors.primary.opacity(0.92))
            }
            .shadow(color: .black.opacity(appColors.scheme == .dark ? 0.35 : 0.08), radius: 12, y: 8)
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
                VerseTileArt()
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
