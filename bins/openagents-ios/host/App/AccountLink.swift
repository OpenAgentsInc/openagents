// The phone on the person's openagents.com account (#11107, #11165): sign
// in with a code, the account's chats, and what Coder runs on the
// computers. Rust owns the state and draws every screen
// (`crates/openagents-mobile/src/account_link.rs`); this file keeps the
// session in Keychain, draws the sign-in QR code, shows local notifications
// with Approve and Deny, and reads in the background.
import BackgroundTasks
import Security
import SwiftUI
import UIKit
import UserNotifications

/// Rust's `account_link::Packet`.
struct LinkPacket: Decodable, Equatable {
    struct Row: Decodable, Equatable, Identifiable {
        let id: String
        let title: String
        let detail: String
    }
    struct Notice: Decodable, Equatable {
        let id: String
        let title: String
        let body: String
        let computer: String
        let item: String
        let question: String?
    }
    let signed_in: Bool
    let label: String?
    let site: String?
    let view: NativeView?
    let qr: ComputersQR?
    let drawer: [Row]
    let running: Int
    let asking: Int
    /// The session to keep, as JSON (sent once).
    let store: JSONValue?
    let forget: Bool?
    let notify: [Notice]?
    let open_url: String?
    let live: Bool

    static func == (a: LinkPacket, b: LinkPacket) -> Bool {
        a.signed_in == b.signed_in && a.label == b.label && a.drawer == b.drawer && a.running == b.running
            && a.asking == b.asking && a.view?.revision == b.view?.revision && a.qr == b.qr
    }
}

/// Any JSON value, kept to hand back to Rust unchanged.
enum JSONValue: Decodable, Equatable {
    case object([String: JSONValue]), array([JSONValue]), string(String), number(Double), bool(Bool), null

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if container.decodeNil() { self = .null }
        else if let value = try? container.decode(Bool.self) { self = .bool(value) }
        else if let value = try? container.decode(Double.self) { self = .number(value) }
        else if let value = try? container.decode(String.self) { self = .string(value) }
        else if let value = try? container.decode([JSONValue].self) { self = .array(value) }
        else { self = .object(try container.decode([String: JSONValue].self)) }
    }

    var any: Any {
        switch self {
        case let .object(map): return map.mapValues { $0.any }
        case let .array(items): return items.map { $0.any }
        case let .string(text): return text
        case let .number(number):
            if number.rounded() == number && abs(number) < 9.0e15 { return NSNumber(value: Int64(number)) }
            return NSNumber(value: number)
        case let .bool(flag): return flag
        case .null: return NSNull()
        }
    }

    /// The value as JSON text.
    var text: String? {
        guard let data = try? JSONSerialization.data(withJSONObject: any) else { return nil }
        return String(data: data, encoding: .utf8)
    }
}

/// The account session in Keychain: this device only, readable after the
/// first unlock so a background read can use it.
enum AccountSessionStore {
    private static let service = "com.openagents.app.account-session"

    private static var query: [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: "openagents.com",
            kSecAttrSynchronizable as String: false,
        ]
    }

    static func load() -> String? {
        var item = query
        item[kSecReturnData as String] = true
        var result: CFTypeRef?
        guard SecItemCopyMatching(item as CFDictionary, &result) == errSecSuccess,
              let data = result as? Data else { return nil }
        return String(data: data, encoding: .utf8)
    }

    @discardableResult
    static func save(_ text: String) -> Bool {
        guard let data = text.data(using: .utf8) else { return false }
        SecItemDelete(query as CFDictionary)
        var item = query
        item[kSecValueData as String] = data
        item[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        return SecItemAdd(item as CFDictionary, nil) == errSecSuccess
    }

    static func delete() { SecItemDelete(query as CFDictionary) }
}

extension MobileBridge {
    /// Tell Rust this phone's name and the session Keychain kept.
    func linkHello() {
        var fields: [String: Any] = ["name": UIDevice.current.name]
        if let session = AccountSessionStore.load() { fields["session"] = session }
        link("hello", fields)
    }

    /// Keep or forget the session, show notices, and open a page, as the
    /// packet asks.
    func settleLink(_ packet: LinkPacket?) {
        guard let packet else { return }
        if let store = packet.store?.text {
            AccountSessionStore.save(store)
            LinkNotifier.shared.ask()
        }
        if packet.forget == true { AccountSessionStore.delete() }
        for notice in packet.notify ?? [] { LinkNotifier.shared.show(notice) }
        if let page = packet.open_url, let url = URL(string: page), url.scheme == "https" {
            UIApplication.shared.open(url)
        }
        if packet.signed_in { LinkBackground.schedule() }
    }
}

/// Local notifications for what Coder runs: finished, failed, or asking,
/// with Approve and Deny on a question.
@MainActor
final class LinkNotifier: NSObject, ObservableObject, UNUserNotificationCenterDelegate {
    static let shared = LinkNotifier()
    static let questionCategory = "openagents.link.question"
    weak var bridge: MobileBridge?
    /// A tap on a notice opens Running.
    @Published var openRunning = 0
    private var asked = false

    func install(_ bridge: MobileBridge) {
        self.bridge = bridge
        let center = UNUserNotificationCenter.current()
        center.delegate = self
        let approve = UNNotificationAction(identifier: "approve", title: "Approve",
                                           options: [.authenticationRequired])
        let deny = UNNotificationAction(identifier: "deny", title: "Deny", options: [.destructive])
        center.setNotificationCategories([
            UNNotificationCategory(identifier: Self.questionCategory, actions: [approve, deny],
                                   intentIdentifiers: [], options: []),
        ])
    }

    /// Ask once for permission to show notices, after sign-in.
    func ask() {
        guard !asked else { return }
        asked = true
        UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound, .badge]) { _, _ in }
    }

    func show(_ notice: LinkPacket.Notice) {
        let content = UNMutableNotificationContent()
        content.title = notice.title
        content.body = notice.body
        content.sound = .default
        content.threadIdentifier = "openagents.link.\(notice.computer)"
        var info: [String: String] = ["computer": notice.computer, "item": notice.item]
        if let question = notice.question {
            info["question"] = question
            content.categoryIdentifier = Self.questionCategory
        }
        content.userInfo = info
        let request = UNNotificationRequest(identifier: notice.id, content: content, trigger: nil)
        UNUserNotificationCenter.current().add(request)
    }

    nonisolated func userNotificationCenter(_ center: UNUserNotificationCenter,
                                            willPresent notification: UNNotification) async
        -> UNNotificationPresentationOptions {
        [.banner, .list, .sound]
    }

    nonisolated func userNotificationCenter(_ center: UNUserNotificationCenter,
                                            didReceive response: UNNotificationResponse) async {
        let info = response.notification.request.content.userInfo
        let action = response.actionIdentifier
        await MainActor.run {
            guard let computer = info["computer"] as? String, let item = info["item"] as? String else { return }
            if (action == "approve" || action == "deny"), let question = info["question"] as? String {
                bridge?.link("answer", ["computer": computer, "item": item, "question": question,
                                        "approve": action == "approve"])
            } else {
                openRunning += 1
            }
        }
    }
}

/// The background read: iOS wakes the app now and then, Rust reads the
/// account once, and anything that finished or asks becomes a notice.
enum LinkBackground {
    static let task = "com.openagents.app.link-refresh"

    /// Register the handler; call before the app finishes launching.
    static func register() {
        BGTaskScheduler.shared.register(forTaskWithIdentifier: task, using: nil) { task in
            Task { @MainActor in
                schedule()
                guard let bridge = LinkNotifier.shared.bridge else {
                    task.setTaskCompleted(success: false)
                    return
                }
                bridge.link("refresh", [:]) { task.setTaskCompleted(success: true) }
            }
        }
    }

    static func schedule() {
        let request = BGAppRefreshTaskRequest(identifier: task)
        request.earliestBeginDate = Date(timeIntervalSinceNow: 15 * 60)
        try? BGTaskScheduler.shared.submit(request)
    }
}

/// The account surface: sign in, the account's chats, one chat, Running.
struct LinkScreen: View {
    @ObservedObject var bridge: MobileBridge
    /// The screen to show first: `account`, `chats`, `running`, or `chat`.
    let screen: String
    var chat: String?
    var menu: (() -> AnyView)?
    @Environment(\.appColors) private var appColors

    var body: some View {
        VStack(spacing: 0) {
            if let menu {
                HStack { menu(); Spacer() }
                    .padding(.horizontal, 16).padding(.top, 4).padding(.bottom, 6)
            }
            if let view = bridge.packet?.link?.view {
                NativeRenderer(node: view.root, revision: view.revision, followTarget: nil, followChanged: nil,
                               surface: { resource, label in
                                   if resource == "link-qr", let qr = bridge.packet?.link?.qr {
                                       return AnyView(InvitationQR(qr: qr)
                                           .frame(width: 220, height: 220)
                                           .frame(maxWidth: .infinity)
                                           .accessibilityLabel(label))
                                   }
                                   return AnyView(EmptyView())
                               },
                               submit: { token, text in bridge.submit("link", token: token, value: text) },
                               activate: { node in bridge.activate("link", view: view, node: node) },
                               activateCurrent: { node in
                                   bridge.activate("link", view: bridge.packet?.link?.view ?? view, node: node)
                               })
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                    .environment(\.nativeFloatingComposer, appColors.background)
            } else {
                Spacer()
            }
        }
        .background(appColors.background.ignoresSafeArea())
        .onAppear { show() }
        .onChange(of: screen) { _, _ in show() }
        .onChange(of: chat) { _, _ in show() }
        .onDisappear { bridge.link("hide") }
    }

    private func show() {
        var fields: [String: Any] = ["screen": screen == "chat" ? "chat" : screen]
        if let chat { fields["id"] = chat }
        bridge.link("show", fields)
    }
}

/// The drawer's account part: Running, and the account's newest chats.
struct LinkDrawerSection: View {
    @ObservedObject var bridge: MobileBridge
    let go: (String) -> Void
    @Environment(\.appColors) private var appColors

    var body: some View {
        let link = bridge.packet?.link
        if link?.signed_in == true {
            Button { go("running") } label: {
                HStack(spacing: 16) {
                    Image(systemName: "bolt.horizontal.circle").font(.system(size: 19)).frame(width: 26)
                    Text("Running").font(.paper(17))
                    Spacer()
                    if let link, link.running > 0 {
                        Text(link.asking > 0 ? "\(link.asking) asking" : "\(link.running)")
                            .font(.paper(14, weight: .semibold))
                            .foregroundStyle(link.asking > 0 ? Color.orange : appColors.secondary)
                    }
                }
                .foregroundStyle(appColors.primary)
                .frame(minHeight: 50)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier("shell-place-running")
            if let rows = link?.drawer, !rows.isEmpty {
                Text("On your account")
                    .font(.paper(13, weight: .semibold))
                    .foregroundStyle(appColors.secondary)
                    .padding(.top, 10)
                ForEach(rows) { row in
                    Button { go("link_chat:\(row.id)") } label: {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(row.title).font(.paper(17)).foregroundStyle(appColors.primary).lineLimit(1)
                            Text(row.detail).font(.paper(13)).foregroundStyle(appColors.secondary).lineLimit(1)
                        }
                        .frame(maxWidth: .infinity, minHeight: 50, alignment: .leading)
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .accessibilityIdentifier("shell-account-chat-\(row.id)")
                }
                Button { go("account_chats") } label: {
                    Text("All account chats…")
                        .font(.paper(17))
                        .foregroundStyle(appColors.secondary)
                        .frame(maxWidth: .infinity, minHeight: 50, alignment: .leading)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("shell-account-chats")
            }
        }
    }
}

/// Settings' first row: Log in, or the account.
struct LinkSettingsRow: View {
    @ObservedObject var bridge: MobileBridge
    let open: () -> Void
    @Environment(\.appColors) private var appColors

    var body: some View {
        let link = bridge.packet?.link
        Button(action: open) {
            HStack {
                Label(link?.signed_in == true ? (link?.label ?? "Your account") : "Log in",
                      systemImage: link?.signed_in == true ? "person.crop.circle.badge.checkmark" : "person.crop.circle")
                Spacer()
                if let site = link?.site { Text(site).font(.paper(.footnote)).foregroundStyle(.secondary) }
                Image(systemName: "chevron.right").font(.paper(.footnote)).foregroundStyle(.tertiary)
            }
        }
        .foregroundStyle(appColors.primary)
        .accessibilityIdentifier("account-link")
    }
}
