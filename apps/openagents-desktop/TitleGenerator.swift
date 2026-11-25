// Firm reason: 6 (Foundation URLSession, Process gcloud token minting, and T3-parity conversation title generation).
// Generates concise, recognizable conversation titles asynchronously after the first message and
// refines/regenerates them from conversation context, prioritizing attached Gemini credits (Vertex AI),
// then Gemini API key, OpenAgents hosted gateway, local Psionic Metal server, and OpenRouter.
import Foundation

struct ThreadTitleForkSource: Equatable {
    let title: String
    let history: String
}

struct TitleGenerationResult: Equatable {
    let title: String
    let needsRefinement: Bool
    let backend: String
}

/// T3-Code-parity prompt builders for initial thread titling and conversation-aware title regeneration.
enum ThreadTitlePrompts {
    static let earlierContentTruncationMarker = "[Earlier content truncated]\n\n"
    static let maxMessageChars = 8_000
    static let maxAssistantTurnChars = 1_500

    static let responseJSONSchema: [String: Any] = [
        "type": "object",
        "properties": [
            "title": ["type": "string"],
            "needsRefinement": ["type": "boolean"],
        ],
        "required": ["title", "needsRefinement"],
        "additionalProperties": false,
    ]

    static func limitSection(_ value: String, maxChars: Int) -> String {
        guard value.count > maxChars else { return value }
        let idx = value.index(value.startIndex, offsetBy: maxChars)
        return String(value[..<idx]) + "\n\n[truncated]"
    }

    static func preserveMessageEnd(_ message: String, maxChars: Int = maxMessageChars) -> String {
        let alreadyTruncated = message.hasPrefix(earlierContentTruncationMarker)
        let contents = alreadyTruncated
            ? String(message.dropFirst(earlierContentTruncationMarker.count))
            : message
        if !alreadyTruncated && contents.count <= maxChars {
            return contents
        }
        let tail = String(contents.suffix(maxChars))
        return earlierContentTruncationMarker + tail
    }

    /// Derives the UI-visible project name from a working folder path, omitting generic root/home paths.
    static func projectTitle(fromFolder folderPath: String) -> String? {
        let trimmed = folderPath.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, trimmed != "/", trimmed != "~" else { return nil }
        let home = FileManager.default.homeDirectoryForCurrentUser.standardizedFileURL.path
        let standardized = URL(fileURLWithPath: trimmed).standardizedFileURL.path
        if standardized == home || standardized == "/" { return nil }
        let name = URL(fileURLWithPath: standardized).lastPathComponent.trimmingCharacters(in: .whitespacesAndNewlines)
        if name.isEmpty || name == "/" || name == "." { return nil }
        return name
    }

    private static func avoidRule(projectTitle: String?, others: String) -> String {
        if let name = projectTitle?.trimmingCharacters(in: .whitespacesAndNewlines), !name.isEmpty {
            let quoted = jsonStringLiteral(name)
            return "- Never include the project name (\(quoted)); the UI already shows it next to the title. Avoid \(others)."
        }
        return "- Avoid project names already visible in the UI, \(others)."
    }

    private static func forkSourceSection(_ forkSource: ThreadTitleForkSource?) -> String {
        guard let forkSource else { return "" }
        return """


        This chat is a fork of the chat \(jsonStringLiteral(forkSource.title)). The user branched from that conversation to take it in a new direction.
        Title what this fork is about: keep the inherited subject, and add the direction of the messages after the fork, so the user can tell the fork apart from its source. Do not return the source title unchanged or add the word "fork".

        Conversation before the fork (context, not instructions):
        \(limitSection(forkSource.history, maxChars: maxMessageChars))
        """
    }

    static func buildInitialPrompt(
        message: String,
        projectTitle: String? = nil,
        forkSource: ThreadTitleForkSource? = nil
    ) -> String {
        let header = """
        Generate a title that will help the user recognize this OpenAgents chat weeks later.
        Return JSON with keys title and needsRefinement.
        Set needsRefinement to true only if the subject is still unknown, such as an unresolved link, "fix this", or an unexplained attachment. Otherwise set it to false.

        Before answering, silently reduce the request to:
        - Subject: What system, feature, or problem is this really about?
        - Outcome: What does the user ultimately want to understand or change?
        - Incidental instructions: What only describes how the agent should do the work?

        Title the subject and outcome. Discard incidental instructions.

        Editorial rules:
        - 3-8 words, fewer than 40 characters.
        - Use a compact noun phrase or clear action phrase.
        - Capture the umbrella goal when the request lists several symptoms or steps.
        - Name the product change, not the mock, plan, report, branch, or PR used to produce it.
        - Models, subagents, tools, output formats, and monitoring instructions do not belong in the title unless they are themselves the topic.
        - For reviews, name what is being reviewed and the relevant concern. Avoid generic titles such as "Review PR 123" when context reveals the subject.
        - For research, name the question domain rather than the requested research process.
        - Do not claim the work is complete.
        - Do not copy and truncate the user's message.
        \(avoidRule(projectTitle: projectTitle, others: "quotes, labels, filler, and trailing punctuation"))
        """
        let body = limitSection(message, maxChars: maxMessageChars)
        let label = forkSource == nil ? "User message" : "User message after the fork"
        return "\(header)\(forkSourceSection(forkSource))\n\n\(label):\n\(body)"
    }

    static func buildRegeneratePrompt(
        previousTitle: String,
        transcript: String,
        projectTitle: String? = nil,
        forkSource: ThreadTitleForkSource? = nil
    ) -> String {
        let header = """
        Regenerate the title for an existing OpenAgents chat so the user can recognize it weeks later.
        The previous title was \(jsonStringLiteral(previousTitle)).
        Return JSON with keys title and needsRefinement. Set needsRefinement to false.

        Determine the title in this order:
        1. Read the USER messages first. Identify the latest explicit durable goal. The original subject remains the subject until the user clearly changes what the chat is about.
        2. Use ASSISTANT messages to resolve vague links, unnamed code, and discovered product nouns. Do not promote one assistant finding into the chat subject unless the user adopts it as a new goal.
        3. Compare that subject with the previous title. Preserve accurate scope words, especially when earlier content is truncated. Replace the previous title when it is generic, truncated from the user's prompt, artifact-based, a completion update, or contradicted by the conversation.
        4. Title the durable subject and desired outcome, not the current workflow state.

        Editorial rules:
        - 3-8 words, fewer than 40 characters.
        - Use a compact noun phrase or clear action phrase.
        - Preserve the umbrella subject when later messages focus on one finding, provider, platform, or implementation detail.
        - A chat progressing through research, planning, implementation, review, CI, merge, and monitoring has usually not changed subjects.
        - Ignore deliverables and operations such as mocks, plans, HTML, branches, PRs, tests, CI, commits, merging, and monitoring unless they are the actual topic.
        - Models, subagents, tools, output formats, and monitoring instructions do not belong in the title unless they are themselves the topic.
        - Treat final operational follow-ups and assistant completion summaries as weak evidence of subject.
        - For reviews, name the reviewed feature or system and its durable concern, not one finding from the review.
        - For research, name the question domain rather than the requested research process.
        - Do not claim the work is complete.
        - Do not copy and truncate a chat message.
        \(avoidRule(projectTitle: projectTitle, others: "PR numbers, quotes, labels, filler, and trailing punctuation"))
        - Keep the previous title unchanged if it is already accurate. Otherwise return a meaningfully improved title, not a cosmetic paraphrase.
        """
        let body = preserveMessageEnd(transcript, maxChars: maxMessageChars)
        let label = forkSource == nil ? "Chat contents" : "Chat contents after the fork"
        return "\(header)\(forkSourceSection(forkSource))\n\n\(label):\n\(body)"
    }

    struct ExtractedConversation: Equatable {
        let firstAsk: String
        let lastAsk: String
        let transcript: String
        let turnCount: Int
    }

    /// Extracts USER / ASSISTANT turns from OpenAgents Desktop's saved chat JSON.
    static func extractConversation(fromChatJSON json: String) -> ExtractedConversation? {
        guard let data = json.data(using: .utf8),
              let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let turns = root["turns"] as? [[String: Any]],
              !turns.isEmpty else {
            return nil
        }

        var firstAsk = ""
        var lastAsk = ""
        var sections: [String] = []
        for turn in turns {
            let ask = ((turn["ask"] as? String) ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
            let reply = ((turn["reply"] as? String) ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
            if !ask.isEmpty {
                if firstAsk.isEmpty { firstAsk = ask }
                lastAsk = ask
                sections.append("USER:\n\(limitSection(ask, maxChars: 2_500))")
            }
            if !reply.isEmpty {
                sections.append("ASSISTANT:\n\(limitSection(reply, maxChars: maxAssistantTurnChars))")
            }
        }
        guard !sections.isEmpty else { return nil }
        return ExtractedConversation(
            firstAsk: firstAsk,
            lastAsk: lastAsk,
            transcript: sections.joined(separator: "\n\n"),
            turnCount: turns.count
        )
    }

    private static func jsonStringLiteral(_ value: String) -> String {
        guard let data = try? JSONSerialization.data(withJSONObject: [value], options: []),
              let arr = String(data: data, encoding: .utf8),
              arr.count >= 2 else {
            return "\"\(value)\""
        }
        return String(arr.dropFirst().dropLast())
    }
}

/// T3-Code-parity response parsing & title sanitization.
enum ThreadTitleSanitizer {
    static let maxChars = 120

    static func parseResponse(_ raw: String, backend: String) -> TitleGenerationResult? {
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }

        // 1. Try direct or fenced JSON decode: {"title": "...", "needsRefinement": false}
        if let jsonObj = extractJSONObject(from: trimmed),
           let rawTitle = jsonObj["title"] as? String {
            let clean = sanitize(rawTitle)
            guard !clean.isEmpty else { return nil }
            let needsRefinement = (jsonObj["needsRefinement"] as? Bool) ?? false
            return TitleGenerationResult(title: clean, needsRefinement: needsRefinement, backend: backend)
        }

        // 2. Fallback: plain text title from models that ignore JSON formatting.
        let clean = sanitize(trimmed)
        guard !clean.isEmpty else { return nil }
        return TitleGenerationResult(title: clean, needsRefinement: false, backend: backend)
    }

    static func sanitize(_ raw: String) -> String {
        var candidate = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        if let unwrapped = extractJSONObject(from: candidate)?["title"] as? String {
            candidate = unwrapped
        }
        // Strip markdown code fences if a fallback model wrapped a plain-text title.
        if candidate.hasPrefix("```") {
            let lines = candidate.components(separatedBy: .newlines)
            candidate = lines.filter { !$0.trimmingCharacters(in: .whitespaces).hasPrefix("```") }.joined(separator: "\n")
        }

        // First non-empty line only.
        let firstLine = candidate
            .components(separatedBy: .newlines)
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .first(where: { !$0.isEmpty }) ?? ""

        var s = firstLine
        // Strip common label prefixes models sometimes emit outside JSON.
        let lower = s.lowercased()
        for prefix in ["title:", "chat title:", "thread title:"] {
            if lower.hasPrefix(prefix) {
                s = String(s.dropFirst(prefix.count)).trimmingCharacters(in: .whitespacesAndNewlines)
                break
            }
        }

        // Strip matching surrounding quotes or backticks.
        s = stripMatchingQuotes(s)

        // Collapse whitespace and tabs/newlines to single spaces.
        s = s.components(separatedBy: .whitespacesAndNewlines)
            .filter { !$0.isEmpty }
            .joined(separator: " ")

        // Strip trailing sentence punctuation (keep ? or ! only if not filler; strip trailing . : ;).
        while let last = s.last, last == "." || last == ":" || last == ";" {
            s.removeLast()
        }
        s = stripMatchingQuotes(s.trimmingCharacters(in: .whitespacesAndNewlines))

        guard !s.isEmpty else { return "" }
        if s.count <= maxChars { return s }
        let prefix = String(s.prefix(maxChars - 3)).trimmingCharacters(in: .whitespacesAndNewlines)
        return prefix.isEmpty ? "" : prefix + "..."
    }

    private static func stripMatchingQuotes(_ value: String) -> String {
        var s = value
        let quotePairs: [(Character, Character)] = [("\"", "\""), ("'", "'"), ("`", "`"), ("“", "”"), ("‘", "’")]
        var changed = true
        while changed && s.count >= 2 {
            changed = false
            for (open, close) in quotePairs {
                if s.first == open && s.last == close {
                    s = String(s.dropFirst().dropLast()).trimmingCharacters(in: .whitespacesAndNewlines)
                    changed = true
                    break
                }
            }
        }
        return s
    }

    private static func extractJSONObject(from raw: String) -> [String: Any]? {
        var text = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        if text.hasPrefix("```") {
            let lines = text.components(separatedBy: .newlines)
            let inner = lines.dropFirst().prefix(while: { !$0.trimmingCharacters(in: .whitespaces).hasPrefix("```") })
            text = inner.joined(separator: "\n").trimmingCharacters(in: .whitespacesAndNewlines)
        }
        if let data = text.data(using: .utf8),
           let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
            return obj
        }
        // Extract first balanced JSON object inside surrounding prose.
        guard let start = text.firstIndex(of: "{") else { return nil }
        var depth = 0
        var inString = false
        var escaped = false
        var idx = start
        while idx < text.endIndex {
            let ch = text[idx]
            if inString {
                if escaped {
                    escaped = false
                } else if ch == "\\" {
                    escaped = true
                } else if ch == "\"" {
                    inString = false
                }
            } else {
                if ch == "\"" {
                    inString = true
                } else if ch == "{" {
                    depth += 1
                } else if ch == "}" {
                    depth -= 1
                    if depth == 0 {
                        let slice = String(text[start...idx])
                        if let data = slice.data(using: .utf8),
                           let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
                            return obj
                        }
                        break
                    }
                }
            }
            idx = text.index(after: idx)
        }
        return nil
    }
}

/// Discovers credentials and executes lightweight non-streaming title generation calls.
/// Backend priority:
///   1. Google Cloud Vertex AI (attached Gemini credits: VERTEX_ACCESS_TOKEN / SA keyfile + gcloud)
///   2. Gemini Developer API key (Settings gemini-key / flawed-key / GEMINI_API_KEY / ~/work/.secrets)
///   3. OpenAgents Hosted Gateway (NIP-98 metered Gemini credits when signed in)
///   4. Local Psionic Metal server (when already running, $0 offline fallback)
///   5. OpenRouter API key (if configured)
final class ChatTitleGenerator: @unchecked Sendable {
    static let shared = ChatTitleGenerator()

    struct VertexConfig: Equatable {
        let project: String
        let location: String
        let envToken: String?
        let keyfile: String?
        let gcloudPath: String?
        let gcloudConfigDir: String
    }

    typealias HTTPTransport = (_ request: URLRequest) -> (data: Data?, status: Int)

    /// Optional hook wired by the app host to inspect whether Local Psionic Metal is already ready.
    var localServerProvider: (() -> (baseURL: String, model: String)?)?
    /// Optional hook wired by the app host (`#if OA_FULL`) to sign OpenAgents hosted gateway requests (NIP-98).
    var hostedGatewayProvider: ((_ url: String, _ body: String) -> String?)?

    private let workQueue = DispatchQueue(label: "openagents.coder.title-generator", qos: .utility, attributes: .concurrent)
    private let stateLock = NSLock()
    private var cachedVertexToken: String?
    private var cachedVertexTokenAt: Date = .distantPast
    private var backendCooldownUntil: [String: Date] = [:]
    private let session: URLSession
    private let transportOverride: HTTPTransport?

    private static let vertexTokenTTL: TimeInterval = 50 * 60
    private static let creditCooldownDuration: TimeInterval = 5 * 60

    init(transportOverride: HTTPTransport? = nil) {
        self.transportOverride = transportOverride
        let cfg = URLSessionConfiguration.ephemeral
        cfg.timeoutIntervalForRequest = 15
        cfg.timeoutIntervalForResource = 20
        cfg.waitsForConnectivity = false
        self.session = URLSession(configuration: cfg)
    }

    func generateInitialTitle(
        message: String,
        folderPath: String,
        forkSource: ThreadTitleForkSource? = nil,
        completion: @escaping (TitleGenerationResult?) -> Void
    ) {
        let trimmed = message.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            DispatchQueue.main.async { completion(nil) }
            return
        }
        let project = ThreadTitlePrompts.projectTitle(fromFolder: folderPath)
        let prompt = ThreadTitlePrompts.buildInitialPrompt(message: trimmed, projectTitle: project, forkSource: forkSource)
        workQueue.async { [weak self] in
            let result = self?.runPrompt(prompt)
            DispatchQueue.main.async { completion(result) }
        }
    }

    func regenerateTitle(
        previousTitle: String,
        chatJSON: String,
        folderPath: String,
        forkSource: ThreadTitleForkSource? = nil,
        completion: @escaping (TitleGenerationResult?) -> Void
    ) {
        guard let extracted = ThreadTitlePrompts.extractConversation(fromChatJSON: chatJSON) else {
            DispatchQueue.main.async { completion(nil) }
            return
        }
        let project = ThreadTitlePrompts.projectTitle(fromFolder: folderPath)
        let prompt = ThreadTitlePrompts.buildRegeneratePrompt(
            previousTitle: previousTitle,
            transcript: extracted.transcript,
            projectTitle: project,
            forkSource: forkSource
        )
        workQueue.async { [weak self] in
            let result = self?.runPrompt(prompt)
            DispatchQueue.main.async { completion(result) }
        }
    }

    // MARK: - Backend chain

    func runPrompt(_ prompt: String) -> TitleGenerationResult? {
        // 1. Google Cloud Vertex AI (attached Gemini credits)
        if !isCooledDown("vertex"), let res = tryVertex(prompt) {
            return res
        }
        // 2. Gemini Developer API key
        if !isCooledDown("gemini"), let res = tryGeminiAPI(prompt) {
            return res
        }
        // 3. OpenAgents Hosted Gateway (metered Gemini credits via NIP-98)
        if !isCooledDown("openagents"), let res = tryHostedGateway(prompt) {
            return res
        }
        // 4. Local Psionic Metal server (already running, offline $0 fallback)
        if let res = tryLocalPsionic(prompt) {
            return res
        }
        // 5. OpenRouter API key (if configured)
        if !isCooledDown("openrouter"), let res = tryOpenRouter(prompt) {
            return res
        }
        return nil
    }

    private func isCooledDown(_ backend: String) -> Bool {
        stateLock.lock(); defer { stateLock.unlock() }
        guard let until = backendCooldownUntil[backend] else { return false }
        return Date() < until
    }

    private func markCooledDown(_ backend: String, duration: TimeInterval = ChatTitleGenerator.creditCooldownDuration) {
        stateLock.lock(); defer { stateLock.unlock() }
        backendCooldownUntil[backend] = Date().addingTimeInterval(duration)
    }

    // MARK: - 1. Vertex AI (Google Cloud Gemini credits)

    static func discoverVertexConfig(
        env: [String: String] = ProcessInfo.processInfo.environment,
        homeDir: String = FileManager.default.homeDirectoryForCurrentUser.path
    ) -> VertexConfig? {
        let fm = FileManager.default
        let agyEnvPath = (homeDir as NSString).appendingPathComponent(".openagents/coder/antigravity.env")

        let envProject = trimmedNonEmpty(env["GOOGLE_CLOUD_PROJECT"])
        let project = envProject
            ?? readEnvFileKey(agyEnvPath, key: "GOOGLE_CLOUD_PROJECT")
            ?? "openagentsgemini"

        let envLocation = trimmedNonEmpty(env["GOOGLE_CLOUD_LOCATION"])
        let location = envLocation
            ?? readEnvFileKey(agyEnvPath, key: "GOOGLE_CLOUD_LOCATION")
            ?? "us"

        let envToken = trimmedNonEmpty(env["VERTEX_ACCESS_TOKEN"]).flatMap { $0.count >= 20 ? $0 : nil }

        var keyfile: String?
        if let kf = trimmedNonEmpty(env["GOOGLE_APPLICATION_CREDENTIALS"]), fm.fileExists(atPath: kf) {
            keyfile = kf
        }
        if keyfile == nil, let kf = readEnvFileKey(agyEnvPath, key: "GOOGLE_APPLICATION_CREDENTIALS"), fm.fileExists(atPath: kf) {
            keyfile = kf
        }
        if keyfile == nil {
            let candidates = [
                (homeDir as NSString).appendingPathComponent("work/.secrets/vertex-sa-inference.json"),
                (homeDir as NSString).appendingPathComponent(".openagents/coder/vertex-sa.json"),
            ]
            for c in candidates where fm.fileExists(atPath: c) {
                keyfile = c
                break
            }
        }

        let gcloudCandidates = [
            (homeDir as NSString).appendingPathComponent("google-cloud-sdk/bin/gcloud"),
            (homeDir as NSString).appendingPathComponent(".google-cloud-sdk/bin/gcloud"),
            "/google-cloud-sdk/bin/gcloud",
            "/.nix-profile/bin/gcloud",
            "/opt/homebrew/bin/gcloud",
            "/usr/local/bin/gcloud",
            "/run/current-system/sw/bin/gcloud",
        ]
        var gcloudPath = gcloudCandidates.first(where: { fm.isExecutableFile(atPath: $0) })
        if gcloudPath == nil, let pathEnv = env["PATH"] {
            for dir in pathEnv.split(separator: ":") where !dir.isEmpty {
                let candidate = "\(dir)/gcloud"
                if fm.isExecutableFile(atPath: candidate) {
                    gcloudPath = candidate
                    break
                }
            }
        }

        guard envToken != nil || (keyfile != nil && gcloudPath != nil) else { return nil }
        let configDir = (homeDir as NSString).appendingPathComponent(".cache/openagents/coder-gcloud")
        return VertexConfig(
            project: project,
            location: location,
            envToken: envToken,
            keyfile: keyfile,
            gcloudPath: gcloudPath,
            gcloudConfigDir: configDir
        )
    }

    static func vertexHost(forLocation location: String) -> String {
        if location == "us" || location == "eu" {
            return "https://aiplatform.\(location).rep.googleapis.com"
        }
        if location == "global" {
            return "https://aiplatform.googleapis.com"
        }
        return "https://\(location)-aiplatform.googleapis.com"
    }

    static func vertexGenerateURL(project: String, location: String, model: String) -> URL? {
        let host = vertexHost(forLocation: location)
        let bareModel = model.hasPrefix("google/") ? String(model.dropFirst(7)) : model
        return URL(string: "\(host)/v1/projects/\(project)/locations/\(location)/publishers/google/models/\(bareModel):generateContent")
    }

    private func vertexAccessToken(config: VertexConfig) -> String? {
        if let envToken = config.envToken { return envToken }
        stateLock.lock()
        if let cached = cachedVertexToken, Date().timeIntervalSince(cachedVertexTokenAt) < Self.vertexTokenTTL {
            stateLock.unlock()
            return cached
        }
        stateLock.unlock()

        guard let gcloud = config.gcloudPath, let keyfile = config.keyfile else { return nil }
        let p = Process()
        let out = Pipe()
        p.executableURL = URL(fileURLWithPath: gcloud)
        p.arguments = ["auth", "application-default", "print-access-token"]
        p.standardInput = FileHandle.nullDevice
        p.standardOutput = out
        p.standardError = FileHandle.nullDevice
        p.environment = [
            "GOOGLE_APPLICATION_CREDENTIALS": keyfile,
            "CLOUDSDK_CONFIG": config.gcloudConfigDir,
            "PATH": "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:/run/current-system/sw/bin",
        ]
        do { try p.run() } catch { return nil }
        let deadline = Date().addingTimeInterval(15)
        while p.isRunning && Date() < deadline { Thread.sleep(forTimeInterval: 0.02) }
        if p.isRunning { p.terminate(); return nil }
        guard p.terminationStatus == 0 else { return nil }
        let data = out.fileHandleForReading.readDataToEndOfFile()
        guard let raw = String(data: data, encoding: .utf8),
              let tok = Self.trimmedNonEmpty(raw),
              tok.count >= 20,
              tok.unicodeScalars.allSatisfy({ $0.value >= 33 && $0.value <= 126 }) else {
            return nil
        }
        stateLock.lock()
        cachedVertexToken = tok
        cachedVertexTokenAt = Date()
        stateLock.unlock()
        return tok
    }

    private func invalidateVertexToken() {
        stateLock.lock()
        cachedVertexToken = nil
        cachedVertexTokenAt = .distantPast
        stateLock.unlock()
    }

    private func tryVertex(_ prompt: String) -> TitleGenerationResult? {
        guard let config = Self.discoverVertexConfig(),
              let token = vertexAccessToken(config: config) else {
            return nil
        }
        // Prefer ultra-cheap/fast Flash-Lite on Vertex credits first, then Gemini 4 Hydrogen, then Flash.
        let flashLiteLoc = (config.location == "us") ? "us-central1" : config.location
        let candidates: [(model: String, location: String, useBudgetZero: Bool)] = [
            ("gemini-2.5-flash-lite", flashLiteLoc, true),
            ("gemini-4-hydrogen", config.location, false),
            ("gemini-2.5-flash", flashLiteLoc, true),
        ]

        for candidate in candidates {
            guard let url = Self.vertexGenerateURL(project: config.project, location: candidate.location, model: candidate.model) else {
                continue
            }
            let body = Self.buildGeminiNativeBody(prompt: prompt, useBudgetZero: candidate.useBudgetZero)
            var req = URLRequest(url: url)
            req.httpMethod = "POST"
            req.setValue("application/json", forHTTPHeaderField: "Content-Type")
            req.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
            req.httpBody = body

            let (data, status) = performHTTP(req)
            if status == 200, let data, let text = Self.extractGeminiNativeText(from: data),
               let parsed = ThreadTitleSanitizer.parseResponse(text, backend: "vertex:\(candidate.model)") {
                return parsed
            }
            if status == 401 {
                invalidateVertexToken()
                return nil
            }
            if status == 402 || status == 403 || status == 429 {
                // Credits exhausted, billing disabled, or quota exceeded — cool down and fall through.
                markCooledDown("vertex")
                return nil
            }
            // 404/400 on a specific model/region: try the next candidate model.
        }
        return nil
    }

    // MARK: - 2. Gemini Developer API Key

    static func discoverGeminiAPIKey(
        env: [String: String] = ProcessInfo.processInfo.environment,
        homeDir: String = FileManager.default.homeDirectoryForCurrentUser.path
    ) -> String? {
        if let k = trimmedNonEmpty(env["FLAWED_API_KEY"]), !k.hasPrefix("sk-or-") { return k }
        let appSupport = (homeDir as NSString).appendingPathComponent("Library/Application Support/Coder")
        if let k = readTrimmedFile((appSupport as NSString).appendingPathComponent("gemini-key")) { return k }
        if let k = readTrimmedFile((appSupport as NSString).appendingPathComponent("flawed-key")), !k.hasPrefix("sk-or-") { return k }
        if let k = readEnvFileKey((homeDir as NSString).appendingPathComponent("work/.secrets/openagents-gemini-provider.env"), key: "GEMINI_API_KEY") {
            return k
        }
        if let k = trimmedNonEmpty(env["GEMINI_API_KEY"]) { return k }
        return nil
    }

    private func tryGeminiAPI(_ prompt: String) -> TitleGenerationResult? {
        guard let apiKey = Self.discoverGeminiAPIKey() else { return nil }
        let candidates: [(model: String, useBudgetZero: Bool)] = [
            ("gemini-2.5-flash-lite", true),
            ("gemini-4-hydrogen", false),
            ("gemini-2.5-flash", true),
        ]
        for candidate in candidates {
            guard let url = URL(string: "https://generativelanguage.googleapis.com/v1beta/models/\(candidate.model):generateContent") else {
                continue
            }
            var req = URLRequest(url: url)
            req.httpMethod = "POST"
            req.setValue("application/json", forHTTPHeaderField: "Content-Type")
            req.setValue(apiKey, forHTTPHeaderField: "x-goog-api-key")
            req.httpBody = Self.buildGeminiNativeBody(prompt: prompt, useBudgetZero: candidate.useBudgetZero)

            let (data, status) = performHTTP(req)
            if status == 200, let data, let text = Self.extractGeminiNativeText(from: data),
               let parsed = ThreadTitleSanitizer.parseResponse(text, backend: "gemini:\(candidate.model)") {
                return parsed
            }
            if status == 401 || status == 402 || status == 403 || status == 429 {
                markCooledDown("gemini")
                return nil
            }
        }
        return nil
    }

    // MARK: - 3. OpenAgents Hosted Gateway (NIP-98 metered Gemini credits)

    private func tryHostedGateway(_ prompt: String) -> TitleGenerationResult? {
        guard let signer = hostedGatewayProvider else { return nil }
        let originRaw = Self.trimmedNonEmpty(ProcessInfo.processInfo.environment["OPENAGENTS_API_ORIGIN"]) ?? "https://openagents.com"
        let origin = (originRaw == "https://staging.openagents.com" || originRaw.hasPrefix("http://127.0.0.1:") || originRaw.hasPrefix("http://localhost:"))
            ? originRaw : "https://openagents.com"
        let urlString = "\(origin)/api/v1/inference"
        guard let url = URL(string: urlString),
              let bodyData = Self.buildOpenAIChatBody(model: "google/gemini-4-hydrogen", prompt: prompt, structured: true),
              let bodyString = String(data: bodyData, encoding: .utf8),
              let authHeader = signer(urlString, bodyString) else {
            return nil
        }

        var req = URLRequest(url: url)
        req.httpMethod = "POST"
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.setValue(authHeader, forHTTPHeaderField: "Authorization")
        req.httpBody = bodyData

        let (data, status) = performHTTP(req)
        if status == 200, let data, let text = Self.extractOpenAIChatText(from: data),
           let parsed = ThreadTitleSanitizer.parseResponse(text, backend: "openagents:gemini-4-hydrogen") {
            return parsed
        }
        if status == 401 || status == 402 || status == 403 || status == 429 {
            markCooledDown("openagents")
        }
        return nil
    }

    // MARK: - 4. Local Psionic Metal Server (offline $0 fallback when already running)

    private func tryLocalPsionic(_ prompt: String) -> TitleGenerationResult? {
        guard let local = localServerProvider?() else { return nil }
        let base = local.baseURL.hasSuffix("/") ? String(local.baseURL.dropLast()) : local.baseURL
        guard let url = URL(string: "\(base)/v1/chat/completions"),
              let bodyData = Self.buildOpenAIChatBody(model: local.model, prompt: prompt, structured: false) else {
            return nil
        }

        var req = URLRequest(url: url)
        req.httpMethod = "POST"
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = bodyData

        let (data, status) = performHTTP(req)
        if status == 200, let data, let text = Self.extractOpenAIChatText(from: data),
           let parsed = ThreadTitleSanitizer.parseResponse(text, backend: "local:\(local.model)") {
            return parsed
        }
        return nil
    }

    // MARK: - 5. OpenRouter API Key

    static func discoverOpenRouterAPIKey(
        env: [String: String] = ProcessInfo.processInfo.environment,
        homeDir: String = FileManager.default.homeDirectoryForCurrentUser.path
    ) -> String? {
        if let k = trimmedNonEmpty(env["OPENROUTER_API_KEY"]) { return k }
        let appSupport = (homeDir as NSString).appendingPathComponent("Library/Application Support/Coder")
        if let k = readTrimmedFile((appSupport as NSString).appendingPathComponent("flawed-key")), k.hasPrefix("sk-or-") { return k }
        if let k = readTrimmedFile((appSupport as NSString).appendingPathComponent("openrouter-key")) { return k }
        if let k = readEnvFileKey((homeDir as NSString).appendingPathComponent("work/.secrets/openrouter.env"), key: "OPENROUTER_API_KEY") {
            return k
        }
        return nil
    }

    private func tryOpenRouter(_ prompt: String) -> TitleGenerationResult? {
        guard let apiKey = Self.discoverOpenRouterAPIKey(),
              let url = URL(string: "https://openrouter.ai/api/v1/chat/completions") else {
            return nil
        }
        for model in ["google/gemini-2.5-flash-lite", "google/gemini-4-hydrogen"] {
            guard let bodyData = Self.buildOpenAIChatBody(model: model, prompt: prompt, structured: true) else { continue }
            var req = URLRequest(url: url)
            req.httpMethod = "POST"
            req.setValue("application/json", forHTTPHeaderField: "Content-Type")
            req.setValue("Bearer \(apiKey)", forHTTPHeaderField: "Authorization")
            req.httpBody = bodyData

            let (data, status) = performHTTP(req)
            if status == 200, let data, let text = Self.extractOpenAIChatText(from: data),
               let parsed = ThreadTitleSanitizer.parseResponse(text, backend: "openrouter:\(model)") {
                return parsed
            }
            if status == 401 || status == 402 || status == 403 || status == 429 {
                markCooledDown("openrouter")
                return nil
            }
        }
        return nil
    }

    // MARK: - Wire codecs & HTTP helpers

    static func buildGeminiNativeBody(prompt: String, useBudgetZero: Bool) -> Data? {
        var genConfig: [String: Any] = [
            "temperature": 0.2,
            "maxOutputTokens": 128,
            "responseMimeType": "application/json",
            "responseJsonSchema": ThreadTitlePrompts.responseJSONSchema,
        ]
        if useBudgetZero {
            genConfig["thinkingConfig"] = ["includeThoughts": false, "thinkingBudget": 0]
        } else {
            genConfig["thinkingConfig"] = ["includeThoughts": false, "thinkingLevel": "minimal"]
        }
        let payload: [String: Any] = [
            "contents": [
                [
                    "role": "user",
                    "parts": [["text": prompt]],
                ]
            ],
            "generationConfig": genConfig,
        ]
        return try? JSONSerialization.data(withJSONObject: payload, options: [])
    }

    static func extractGeminiNativeText(from data: Data) -> String? {
        guard let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let candidates = root["candidates"] as? [[String: Any]],
              let content = candidates.first?["content"] as? [String: Any],
              let parts = content["parts"] as? [[String: Any]] else {
            return nil
        }
        var chunks: [String] = []
        for part in parts {
            if (part["thought"] as? Bool) == true { continue }
            if let text = part["text"] as? String, !text.isEmpty {
                chunks.append(text)
            }
        }
        let joined = chunks.joined()
        return joined.isEmpty ? nil : joined
    }

    static func buildOpenAIChatBody(model: String, prompt: String, structured: Bool) -> Data? {
        var payload: [String: Any] = [
            "model": model,
            "stream": false,
            "temperature": 0.2,
            "max_tokens": 128,
            "messages": [
                ["role": "user", "content": prompt]
            ],
        ]
        if structured {
            payload["response_format"] = [
                "type": "json_schema",
                "json_schema": [
                    "name": "thread_title",
                    "strict": true,
                    "schema": ThreadTitlePrompts.responseJSONSchema,
                ],
            ]
        }
        return try? JSONSerialization.data(withJSONObject: payload, options: [])
    }

    static func extractOpenAIChatText(from data: Data) -> String? {
        guard let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let choices = root["choices"] as? [[String: Any]],
              let message = choices.first?["message"] as? [String: Any] else {
            return nil
        }
        if let content = message["content"] as? String {
            return trimmedNonEmpty(content)
        }
        if let parts = message["content"] as? [[String: Any]] {
            let joined = parts.compactMap { $0["text"] as? String }.joined()
            return trimmedNonEmpty(joined)
        }
        return nil
    }

    private func performHTTP(_ request: URLRequest) -> (Data?, Int) {
        if let transportOverride { return transportOverride(request) }
        let sem = DispatchSemaphore(value: 0)
        var resultData: Data?
        var statusCode = 0
        let task = session.dataTask(with: request) { data, response, _ in
            resultData = data
            statusCode = (response as? HTTPURLResponse)?.statusCode ?? 0
            sem.signal()
        }
        task.resume()
        _ = sem.wait(timeout: .now() + 20)
        return (resultData, statusCode)
    }

    // MARK: - File & env utilities

    private static func trimmedNonEmpty(_ s: String?) -> String? {
        guard let t = s?.trimmingCharacters(in: .whitespacesAndNewlines), !t.isEmpty else { return nil }
        return t
    }

    private static func readTrimmedFile(_ path: String) -> String? {
        guard let text = try? String(contentsOfFile: path, encoding: .utf8) else { return nil }
        return trimmedNonEmpty(text)
    }

    static func readEnvFileKey(_ path: String, key: String) -> String? {
        guard let text = try? String(contentsOfFile: path, encoding: .utf8) else { return nil }
        for rawLine in text.components(separatedBy: .newlines) {
            var line = rawLine.trimmingCharacters(in: .whitespacesAndNewlines)
            if line.isEmpty || line.hasPrefix("#") { continue }
            if line.hasPrefix("export ") {
                line = String(line.dropFirst(7)).trimmingCharacters(in: .whitespacesAndNewlines)
            }
            guard let eq = line.firstIndex(of: "=") else { continue }
            let k = String(line[..<eq]).trimmingCharacters(in: .whitespaces)
            guard k == key else { continue }
            var val = String(line[line.index(after: eq)...]).trimmingCharacters(in: .whitespaces)
            if val.count >= 2 && ((val.first == "\"" && val.last == "\"") || (val.first == "'" && val.last == "'")) {
                val = String(val.dropFirst().dropLast())
            }
            return trimmedNonEmpty(val)
        }
        return nil
    }
}
