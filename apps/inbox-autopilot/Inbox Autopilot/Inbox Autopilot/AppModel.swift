import Foundation
import Combine

@MainActor
final class AppModel: ObservableObject {
    @Published var daemonConnected = false
    @Published var daemonStatusText = "Disconnected"
    @Published var gmailConnected = false
    @Published var chatGPTConnected = false

    @Published var threads: [ThreadSummary] = []
    @Published var searchText = ""
    @Published var selectedThreadID: String?
    @Published var threadDetail: ThreadDetailResponse?
    @Published var threadAudit: AuditResponse?

    @Published var pendingDrafts: [DraftRecord] = []
    @Published var events: [EventRecord] = []
    @Published var templateSuggestions: [TemplateSuggestion] = []
    @Published var draftQualityReport: DraftQualityReport?
    @Published var availableUpdate: AppReleaseInfo?
    @Published var isCheckingForUpdates = false
    @Published var lastUpdateCheckAt: Date?

    @Published var settings = SettingsResponse(
        privacyMode: .hybrid,
        backfillDays: 90,
        allowedRecipientDomains: [],
        attachmentStorageMode: .metadata,
        signature: nil,
        templateScheduling: nil,
        templateReportDelivery: nil,
        syncIntervalSeconds: 60
    )

    @Published var chatGPTAPIKeyInput = ""
    @Published var manualGmailCodeInput = ""

    @Published var isBusy = false
    @Published var notice: String?
    @Published var errorMessage: String?
    @Published var needsOnboarding = false

    private let client = DaemonAPIClient()
    private let notifications = NotificationManager.shared
    private let defaultUpdateFeedURL = URL(string: "https://api.github.com/repos/OpenAgentsInc/openagents/releases/latest")!
    private let updateCheckCooldown: TimeInterval = 60 * 60 * 12
    private let updateLastCheckedKey = "inboxautopilot.lastUpdateCheckAt"
    private let updateFeedOverrideKey = "inboxautopilot.updateFeedURL"
    private var eventTask: Task<Void, Never>?

    func startup() async {
        lastUpdateCheckAt = UserDefaults.standard.object(forKey: updateLastCheckedKey) as? Date
        await notifications.requestAuthorizationIfNeeded()
        if shouldRunBackgroundUpdateCheck(now: Date()) {
            await checkForUpdates(manual: false)
        }
        await refreshHealth()
        guard daemonConnected else {
            return
        }

        await refreshEverything()
        startEventStream()
        needsOnboarding = !gmailConnected
    }

    func refreshHealth() async {
        do {
            let health = try await client.health()
            daemonConnected = (health.status == "ok")
            gmailConnected = health.connectedGmail
            chatGPTConnected = health.connectedChatgpt
            daemonStatusText = daemonConnected ? "Connected" : "Disconnected"
        } catch {
            daemonConnected = false
            daemonStatusText = "Disconnected"
            errorMessage = error.localizedDescription
        }
    }

    func refreshEverything() async {
        guard daemonConnected else {
            return
        }

        await withTaskGroup(of: Void.self) { group in
            group.addTask { await self.refreshAuthStatus() }
            group.addTask { await self.refreshThreads() }
            group.addTask { await self.refreshPendingDrafts() }
            group.addTask { await self.refreshEvents() }
            group.addTask { await self.refreshSettings() }
        }

        if let selectedThreadID {
            await openThread(id: selectedThreadID)
        }
    }

    func refreshAuthStatus() async {
        do {
            let gmail = try await client.gmailStatus()
            let chatgpt = try await client.chatGPTStatus()
            gmailConnected = gmail.connected
            chatGPTConnected = chatgpt.connected
            needsOnboarding = !gmailConnected
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func refreshThreads() async {
        guard daemonConnected else { return }
        do {
            let threads = try await client.listThreads(search: searchText)
            self.threads = threads
            if selectedThreadID == nil {
                selectedThreadID = threads.first?.id
            }
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func refreshPendingDrafts() async {
        guard daemonConnected else { return }
        do {
            pendingDrafts = try await client.pendingDrafts()
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func refreshEvents() async {
        guard daemonConnected else { return }
        do {
            events = try await client.events(limit: 300)
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func refreshSettings() async {
        guard daemonConnected else { return }
        do {
            settings = try await client.settings()
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func saveSettings() async {
        guard daemonConnected else { return }
        isBusy = true
        defer { isBusy = false }
        do {
            let update = UpdateSettingsRequest(
                privacyMode: settings.privacyMode,
                backfillDays: settings.backfillDays,
                allowedRecipientDomains: settings.allowedRecipientDomains,
                attachmentStorageMode: settings.attachmentStorageMode,
                signature: settings.signature,
                templateScheduling: settings.templateScheduling,
                templateReportDelivery: settings.templateReportDelivery,
                syncIntervalSeconds: settings.syncIntervalSeconds
            )
            settings = try await client.updateSettings(update)
            notice = "Settings saved."
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func runBackfill() async {
        guard daemonConnected else { return }
        isBusy = true
        defer { isBusy = false }

        do {
            let result = try await client.startBackfill(days: settings.backfillDays)
            notice = "Backfill complete: \(result.importedThreads) threads, \(result.importedMessages) messages."
            await refreshEverything()
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func syncNow() async {
        guard daemonConnected else { return }
        isBusy = true
        defer { isBusy = false }

        do {
            let result = try await client.syncNow()
            notice = "Sync complete: \(result.importedThreads) threads, \(result.importedMessages) messages."
            await refreshEverything()
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func openThread(id: String?) async {
        guard daemonConnected else { return }
        guard let id else {
            selectedThreadID = nil
            threadDetail = nil
            threadAudit = nil
            return
        }

        selectedThreadID = id
        do {
            async let detail = client.threadDetail(id: id)
            async let audit = client.audit(threadID: id)
            threadDetail = try await detail
            threadAudit = try await audit
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func generateDraftForSelectedThread() async {
        guard let threadID = selectedThreadID else { return }
        isBusy = true
        defer { isBusy = false }

        do {
            _ = try await client.generateDraft(threadID: threadID)
            notice = "Draft generated."
            await openThread(id: threadID)
            await refreshPendingDrafts()
            await refreshThreads()
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func approveAndSendSelectedThread() async {
        guard let threadID = selectedThreadID else { return }
        isBusy = true
        defer { isBusy = false }

        do {
            let result = try await client.approveAndSend(threadID: threadID)
            notice = "Email sent. Gmail message id: \(result.gmailMessageID)"
            await openThread(id: threadID)
            await refreshPendingDrafts()
            await refreshThreads()
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func markNeedsHuman(draftID: String) async {
        isBusy = true
        defer { isBusy = false }

        do {
            try await client.markDraftNeedsHuman(draftID: draftID)
            notice = "Draft marked as needs human review."
            await refreshPendingDrafts()
            if let selectedThreadID {
                await openThread(id: selectedThreadID)
            }
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func exportSelectedAudit() async {
        guard let threadID = selectedThreadID else { return }
        do {
            let exported = try await client.exportAudit(threadID: threadID)
            notice = "Exported \(exported.exportedEvents) events to \(exported.path)"
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func connectChatGPT() async {
        let key = chatGPTAPIKeyInput.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !key.isEmpty else {
            errorMessage = "Enter a ChatGPT/OpenAI API key first."
            return
        }

        isBusy = true
        defer { isBusy = false }

        do {
            try await client.connectChatGPT(apiKey: key)
            chatGPTAPIKeyInput = ""
            notice = "ChatGPT connected."
            await refreshAuthStatus()
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func connectGmail() async {
        isBusy = true
        defer { isBusy = false }

        do {
            let redirectURI = "inboxautopilot://oauth/callback"
            let authInfo = try await client.gmailAuthURL(redirectURI: redirectURI)
            guard let url = URL(string: authInfo.url) else {
                throw URLError(.badURL)
            }

            let oauthSession = GmailOAuthSession()
            let callback = try await oauthSession.begin(url: url, callbackScheme: "inboxautopilot")

            guard
                let components = URLComponents(url: callback, resolvingAgainstBaseURL: false),
                let code = components.queryItems?.first(where: { $0.name == "code" })?.value
            else {
                throw OAuthConnectError.missingCode
            }

            let request = GmailAuthRequest(code: code, redirectURI: redirectURI, codeVerifier: nil)
            try await client.exchangeGmailCode(request)
            notice = "Gmail connected successfully."
            await refreshAuthStatus()
            needsOnboarding = false
        } catch {
            errorMessage = error.localizedDescription + " If callback URL registration is missing, copy the `code` from the redirect URL and use manual exchange below."
        }
    }

    func connectGmailWithManualCode() async {
        let code = manualGmailCodeInput.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !code.isEmpty else {
            errorMessage = "Paste an OAuth code first."
            return
        }

        isBusy = true
        defer { isBusy = false }

        do {
            let request = GmailAuthRequest(
                code: code,
                redirectURI: "inboxautopilot://oauth/callback",
                codeVerifier: nil
            )
            try await client.exchangeGmailCode(request)
            manualGmailCodeInput = ""
            notice = "Gmail connected successfully."
            await refreshAuthStatus()
            needsOnboarding = false
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func mineTemplateSuggestions() async {
        guard daemonConnected else { return }
        do {
            templateSuggestions = try await client.mineTemplates()
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func refreshDraftQualityReport(limitPerCategory: Int = 200, threshold: Double = 0.35) async {
        guard daemonConnected else { return }
        do {
            draftQualityReport = try await client.draftEditRate(
                limitPerCategory: limitPerCategory,
                threshold: threshold
            )
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func checkForUpdates(manual: Bool = true) async {
        if isCheckingForUpdates {
            return
        }

        isCheckingForUpdates = true
        defer { isCheckingForUpdates = false }

        do {
            let latest = try await fetchLatestRelease()
            let current = AppVersion.current()

            if latest.isNewer(than: current) {
                availableUpdate = latest
                if manual {
                    notice = "Update \(latest.version) is available."
                }
            } else {
                availableUpdate = nil
                if manual {
                    notice = "Inbox Autopilot is up to date (\(current.display))."
                }
            }

            let now = Date()
            lastUpdateCheckAt = now
            UserDefaults.standard.set(now, forKey: updateLastCheckedKey)
        } catch {
            if manual {
                errorMessage = "Update check failed: \(error.localizedDescription)"
            } else {
                print("background update check failed: \(error.localizedDescription)")
            }
        }
    }

    func deleteLocalCorpus() async {
        guard daemonConnected else { return }
        isBusy = true
        defer { isBusy = false }

        do {
            try await client.deleteLocalCorpus()
            notice = "Local corpus deleted."
            threads = []
            pendingDrafts = []
            events = []
            threadDetail = nil
            threadAudit = nil
            selectedThreadID = nil
            templateSuggestions = []
            draftQualityReport = nil
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func factoryReset() async {
        guard daemonConnected else { return }
        isBusy = true
        defer { isBusy = false }

        do {
            try await client.factoryReset()
            notice = "Factory reset complete."
            gmailConnected = false
            chatGPTConnected = false
            manualGmailCodeInput = ""
            chatGPTAPIKeyInput = ""
            templateSuggestions = []
            draftQualityReport = nil
            await refreshEverything()
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func clearMessages() {
        notice = nil
        errorMessage = nil
    }

    var currentVersionDisplay: String {
        AppVersion.current().display
    }

    private func shouldRunBackgroundUpdateCheck(now: Date) -> Bool {
        guard let lastUpdateCheckAt else {
            return true
        }
        return now.timeIntervalSince(lastUpdateCheckAt) >= updateCheckCooldown
    }

    private func resolvedUpdateFeedURL() -> URL {
        if let raw = UserDefaults.standard.string(forKey: updateFeedOverrideKey),
           let customURL = URL(string: raw)
        {
            return customURL
        }
        return defaultUpdateFeedURL
    }

    private func fetchLatestRelease() async throws -> AppReleaseInfo {
        var request = URLRequest(url: resolvedUpdateFeedURL())
        request.httpMethod = "GET"
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        request.setValue("InboxAutopilot/\(AppVersion.current().display)", forHTTPHeaderField: "User-Agent")

        let (data, response) = try await URLSession.shared.data(for: request)
        guard let http = response as? HTTPURLResponse else {
            throw UpdateCheckError.nonHTTPResponse
        }
        guard 200..<300 ~= http.statusCode else {
            let body = String(data: data, encoding: .utf8) ?? ""
            throw UpdateCheckError.httpStatus(http.statusCode, body)
        }

        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601

        if let manifest = try? decoder.decode(UpdateFeedManifest.self, from: data) {
            guard let downloadURL = URL(string: manifest.downloadURL) else {
                throw UpdateCheckError.invalidPayload("update manifest is missing a valid download_url")
            }
            let releaseNotesURL = manifest.releaseNotesURL.flatMap(URL.init(string:))
            return AppReleaseInfo(
                version: manifest.version,
                build: manifest.build,
                downloadURL: downloadURL,
                releaseNotesURL: releaseNotesURL,
                notes: manifest.notes,
                publishedAt: manifest.publishedAt
            )
        }

        let githubRelease = try decoder.decode(GitHubReleasePayload.self, from: data)
        let version = githubRelease.tagName
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .trimmingPrefix("v")
            .trimmingPrefix("V")
        let downloadURL = githubRelease.assets.first?.browserDownloadURL ?? githubRelease.htmlURL

        return AppReleaseInfo(
            version: version,
            build: nil,
            downloadURL: downloadURL,
            releaseNotesURL: githubRelease.htmlURL,
            notes: githubRelease.body,
            publishedAt: githubRelease.publishedAt
        )
    }

    private func startEventStream() {
        eventTask?.cancel()
        eventTask = Task {
            do {
                let stream = try await client.eventStream()
                for try await event in stream {
                    guard !Task.isCancelled else { return }
                    await MainActor.run {
                        self.events.append(event)
                        if self.events.count > 1_000 {
                            self.events.removeFirst(self.events.count - 1_000)
                        }
                    }

                    if event.eventType == "draft_created" {
                        await MainActor.run {
                            notifications.notify(
                                title: "Draft ready",
                                body: "A new draft was created\(event.threadID.map { " for thread \($0)" } ?? "")."
                            )
                        }
                    } else if event.eventType == "draft_marked_needs_human" {
                        await MainActor.run {
                            notifications.notify(
                                title: "Needs human review",
                                body: "A draft was flagged for manual handling."
                            )
                        }
                    }

                    if ["draft_created", "classification_completed", "email_sent", "sync_backfill_completed", "sync_incremental_completed"].contains(event.eventType) {
                        await refreshThreads()
                        await refreshPendingDrafts()
                        if let selectedThreadID, selectedThreadID == event.threadID {
                            await openThread(id: selectedThreadID)
                        }
                    }
                }
            } catch {
                await MainActor.run {
                    self.errorMessage = error.localizedDescription
                }
            }
        }
    }
}

enum OAuthConnectError: LocalizedError {
    case missingCode

    var errorDescription: String? {
        switch self {
        case .missingCode:
            return "OAuth callback did not include an auth code."
        }
    }
}

struct AppReleaseInfo {
    let version: String
    let build: String?
    let downloadURL: URL
    let releaseNotesURL: URL?
    let notes: String?
    let publishedAt: Date?

    func isNewer(than current: AppVersion) -> Bool {
        switch AppVersion.compare(version, current.version) {
        case .orderedDescending:
            return true
        case .orderedAscending:
            return false
        case .orderedSame:
            guard
                let newBuild = build.flatMap(Int.init),
                let currentBuild = current.build.flatMap(Int.init)
            else {
                return false
            }
            return newBuild > currentBuild
        }
    }
}

struct AppVersion {
    let version: String
    let build: String?

    static func current() -> Self {
        let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String
        let build = Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String
        return AppVersion(
            version: version ?? "0.0.0",
            build: build
        )
    }

    var display: String {
        if let build, !build.isEmpty {
            return "\(version) (\(build))"
        }
        return version
    }

    static func compare(_ lhs: String, _ rhs: String) -> ComparisonResult {
        let lhsComponents = normalizedVersionComponents(lhs)
        let rhsComponents = normalizedVersionComponents(rhs)
        let maxCount = max(lhsComponents.count, rhsComponents.count)

        for index in 0..<maxCount {
            let left = index < lhsComponents.count ? lhsComponents[index] : 0
            let right = index < rhsComponents.count ? rhsComponents[index] : 0
            if left < right {
                return .orderedAscending
            }
            if left > right {
                return .orderedDescending
            }
        }

        return .orderedSame
    }

    private static func normalizedVersionComponents(_ raw: String) -> [Int] {
        raw.trimmingCharacters(in: .whitespacesAndNewlines)
            .trimmingPrefix("v")
            .trimmingPrefix("V")
            .split(separator: ".")
            .map { component in
                let digits = component.filter(\.isNumber)
                return Int(digits) ?? 0
            }
    }
}

private struct UpdateFeedManifest: Decodable {
    let version: String
    let build: String?
    let downloadURL: String
    let releaseNotesURL: String?
    let notes: String?
    let publishedAt: Date?

    enum CodingKeys: String, CodingKey {
        case version
        case build
        case downloadURL = "download_url"
        case releaseNotesURL = "release_notes_url"
        case notes
        case publishedAt = "published_at"
    }
}

private struct GitHubReleasePayload: Decodable {
    let tagName: String
    let htmlURL: URL
    let body: String?
    let publishedAt: Date?
    let assets: [GitHubAsset]

    enum CodingKeys: String, CodingKey {
        case tagName = "tag_name"
        case htmlURL = "html_url"
        case body
        case publishedAt = "published_at"
        case assets
    }
}

private struct GitHubAsset: Decodable {
    let browserDownloadURL: URL

    enum CodingKeys: String, CodingKey {
        case browserDownloadURL = "browser_download_url"
    }
}

private enum UpdateCheckError: LocalizedError {
    case nonHTTPResponse
    case httpStatus(Int, String)
    case invalidPayload(String)

    var errorDescription: String? {
        switch self {
        case .nonHTTPResponse:
            return "Update feed returned a non-HTTP response."
        case .httpStatus(let status, let body):
            return "Update feed returned status \(status): \(body)"
        case .invalidPayload(let message):
            return "Update feed payload is invalid: \(message)"
        }
    }
}

private extension String {
    func trimmingPrefix(_ prefix: String) -> String {
        guard hasPrefix(prefix) else { return self }
        return String(dropFirst(prefix.count))
    }
}
