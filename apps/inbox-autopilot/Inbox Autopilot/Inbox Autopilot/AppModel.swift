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
    private var eventTask: Task<Void, Never>?

    func startup() async {
        await notifications.requestAuthorizationIfNeeded()
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
            await refreshEverything()
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func clearMessages() {
        notice = nil
        errorMessage = nil
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
