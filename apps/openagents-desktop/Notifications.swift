// Firm reason: 6 (AppKit/UserNotifications platform effects: UNUserNotificationCenter, NSSound chimes, NSApp.dockTile badge).
// Turn-completion & needs-input OS notifications, subtle audio chimes, and Dock badge count (GitHub #13874).
// All policy decisions and string/badge formatting live in Coax (page/notify.coax).
import AppKit
import UserNotifications

enum CoderNotifyMode: Int32, CaseIterable {
    case off = 0
    case notifications = 1
    case sound = 2
    case notificationsAndSound = 3

    static let defaultsKey = "notificationMode"

    init(clamping raw: Int32) {
        if raw <= 0 { self = .off }
        else if raw == 1 { self = .notifications }
        else if raw == 2 { self = .sound }
        else { self = .notificationsAndSound }
    }

    var needsOS: Bool { self == .notifications || self == .notificationsAndSound }

    static var current: CoderNotifyMode {
        get {
            guard UserDefaults.standard.object(forKey: defaultsKey) != nil else { return .notificationsAndSound }
            return CoderNotifyMode(clamping: Int32(UserDefaults.standard.integer(forKey: defaultsKey)))
        }
        set {
            UserDefaults.standard.set(Int(newValue.rawValue), forKey: defaultsKey)
        }
    }
}

enum CoderNotifyEvent {
    static let none: Int32 = 0
    static let turnDone: Int32 = 1
    static let turnFailed: Int32 = 2
    static let needInput: Int32 = 3
    static let goalChange: Int32 = 4
}

enum CoderSoundKind {
    static let none: Int32 = 0
    static let complete: Int32 = 1
    static let attention: Int32 = 2
}

final class CoderNotifications: NSObject, UNUserNotificationCenterDelegate {
    static let shared = CoderNotifications()

    /// Invoked on the main actor when the user clicks a local chat notification.
    var onSelectChat: ((String) -> Void)?

    private var authRequested = false

    func install() {
        guard Bundle.main.bundleIdentifier != nil else { return }
        UNUserNotificationCenter.current().delegate = self
    }

    func requestAuthorizationIfNeeded() {
        guard Bundle.main.bundleIdentifier != nil, !TestLaunch.active, !authRequested, CoderNotifyMode.current.needsOS else { return }
        authRequested = true
        UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound, .badge]) { _, _ in }
    }

    func deliver(event: Int32, chatID: String, chatTitle: String, detail: String, appFocused: Bool, isActiveChat: Bool) {
        let mode = CoderNotifyMode.current.rawValue
        let bits = cd_notify_decide(mode, event, appFocused ? 1 : 0, isActiveChat ? 1 : 0)
        guard bits != 0 else { return }
        let fireOS = (bits & 1) != 0
        let fireSound = (bits & 2) != 0
        let soundKind = bits >> 2

        if fireSound {
            playChime(kind: soundKind)
        }
        if fireOS {
            postOSNotification(event: event, chatID: chatID, chatTitle: chatTitle, detail: detail)
        }
    }

    func playChime(kind: Int32) {
        guard !TestLaunch.active, kind != CoderSoundKind.none else { return }
        let names: [String] = kind == CoderSoundKind.attention ? ["Tink", "Ping"] : ["Glass", "Pop"]
        DispatchQueue.main.async {
            for name in names {
                if let sound = NSSound(named: NSSound.Name(name)) {
                    sound.volume = 0.45
                    sound.play()
                    return
                }
            }
        }
    }

    func updateDockBadge(count: Int) {
        var buf = [CChar](repeating: 0, count: 16)
        let n = cd_notify_badge_label(Int32(max(0, count)), &buf, Int32(buf.count))
        let label = n > 0 ? String(cString: buf) : ""
        DispatchQueue.main.async {
            NSApp.dockTile.badgeLabel = label.isEmpty ? nil : label
            NSApp.dockTile.display()
        }
    }

    private func postOSNotification(event: Int32, chatID: String, chatTitle: String, detail: String) {
        guard Bundle.main.bundleIdentifier != nil, !TestLaunch.active else { return }
        requestAuthorizationIfNeeded()
        var titleBuf = [CChar](repeating: 0, count: 256)
        var bodyBuf = [CChar](repeating: 0, count: 512)
        _ = cd_notify_title(event, chatTitle, &titleBuf, Int32(titleBuf.count))
        _ = cd_notify_body(event, detail, &bodyBuf, Int32(bodyBuf.count))

        let content = UNMutableNotificationContent()
        content.title = String(cString: titleBuf)
        content.body = String(cString: bodyBuf)
        // Sound is played directly via playChime(_:) according to CoderNotifyMode so we never
        // double-chime or play sound in Notifications-only mode.
        content.sound = nil
        if !chatID.isEmpty {
            content.userInfo = ["chatId": chatID]
            content.threadIdentifier = chatID
        }
        let id = "oa.chat.\(chatID.isEmpty ? "active" : chatID).\(event).\(UUID().uuidString)"
        let request = UNNotificationRequest(identifier: id, content: content, trigger: nil)
        UNUserNotificationCenter.current().add(request) { _ in }
    }

    // MARK: - UNUserNotificationCenterDelegate

    func userNotificationCenter(_ center: UNUserNotificationCenter, willPresent notification: UNNotification) async -> UNNotificationPresentationOptions {
        [.banner, .list]
    }

    func userNotificationCenter(_ center: UNUserNotificationCenter, didReceive response: UNNotificationResponse) async {
        let userInfo = response.notification.request.content.userInfo
        if let chatID = userInfo["chatId"] as? String, !chatID.isEmpty {
            await MainActor.run {
                NSApp.activate(ignoringOtherApps: true)
                self.onSelectChat?(chatID)
            }
            return
        }
        #if OA_FULL
        if AppAuth.noticeOpens(userInfo) {
            await MacAccount.shared.showSecurity()
        }
        #endif
    }
}

extension CoderWindow {
    var isAppFocused: Bool { NSApp.isActive && window.isVisible && window.isKeyWindow }

    func chatDisplayTitle(_ id: String?) -> String {
        guard let id, !id.isEmpty else { return "OpenAgents" }
        if id == Foreman.chatID { return Foreman.title }
        return store.title(id) ?? "Chat"
    }

    func updateDockBadge() {
        let total = unread.union(needsAttention).count
        CoderNotifications.shared.updateDockBadge(count: total)
    }

    func clearViewedChatBadges(_ id: String?) {
        guard let id, !id.isEmpty else { return }
        let removedUnread = unread.remove(id) != nil
        if id == chat && cd_permit_open(page) == 0 {
            needsAttention.remove(id)
        }
        if removedUnread { cd_chats(page, chatLines) }
        updateDockBadge()
    }

    func openFromNotification(_ chatID: String) {
        show()
        if !chatID.isEmpty {
            open(chatID)
            clearViewedChatBadges(chatID)
        }
    }

    /// Checks a run's page for newly opened permission/question cards or actionable /goal transitions.
    func checkRunNotifications(_ r: CoderRun) {
        let isShown = r === run
        let cid = (isShown ? chat : r.chat) ?? ""
        let isActive = isShown && (cid.isEmpty || cid == chat)
        let title = chatDisplayTitle(cid)

        let permitOpen = cd_permit_open(r.page) != 0
        if permitOpen && !r.lastPermitOpen {
            var pbuf = [CChar](repeating: 0, count: 256)
            let pn = cd_permit_title(r.page, &pbuf, Int32(pbuf.count))
            let detail = pn > 0 ? String(cString: pbuf) : ""
            if !cid.isEmpty { needsAttention.insert(cid) }
            CoderNotifications.shared.deliver(event: CoderNotifyEvent.needInput, chatID: cid, chatTitle: title, detail: detail, appFocused: isAppFocused, isActiveChat: isActive)
            updateDockBadge()
        } else if !permitOpen && r.lastPermitOpen {
            if !cid.isEmpty { needsAttention.remove(cid) }
            updateDockBadge()
        }
        r.lastPermitOpen = permitOpen

        let gs = cd_goal_status(r.page)
        if gs != r.lastGoalStatus {
            let ev = cd_notify_goal_event(r.lastGoalStatus, gs)
            r.lastGoalStatus = gs
            if ev != CoderNotifyEvent.none {
                var obuf = [CChar](repeating: 0, count: 256)
                let on = cd_goal_objective(r.page, &obuf, Int32(obuf.count))
                let detail = on > 0 ? String(cString: obuf) : ""
                if ev == CoderNotifyEvent.needInput {
                    if !cid.isEmpty { needsAttention.insert(cid) }
                } else if !cid.isEmpty && (!isActive || !isAppFocused) {
                    unread.insert(cid)
                }
                CoderNotifications.shared.deliver(event: ev, chatID: cid, chatTitle: title, detail: detail, appFocused: isAppFocused, isActiveChat: isActive)
                updateDockBadge()
            }
        }
    }

    func notifyTurnCompleted(chatID: String?, isActiveChat: Bool) {
        let cid = chatID ?? ""
        if !cid.isEmpty && (!isActiveChat || !isAppFocused) {
            unread.insert(cid)
        }
        CoderNotifications.shared.deliver(
            event: CoderNotifyEvent.turnDone,
            chatID: cid,
            chatTitle: chatDisplayTitle(cid),
            detail: "",
            appFocused: isAppFocused,
            isActiveChat: isActiveChat
        )
        updateDockBadge()
    }

    func notifyTurnFailed(chatID: String?, detail: String, isActiveChat: Bool) {
        let cid = chatID ?? ""
        if !cid.isEmpty && (!isActiveChat || !isAppFocused) {
            unread.insert(cid)
        }
        CoderNotifications.shared.deliver(
            event: CoderNotifyEvent.turnFailed,
            chatID: cid,
            chatTitle: chatDisplayTitle(cid),
            detail: detail,
            appFocused: isAppFocused,
            isActiveChat: isActiveChat
        )
        updateDockBadge()
    }
}
