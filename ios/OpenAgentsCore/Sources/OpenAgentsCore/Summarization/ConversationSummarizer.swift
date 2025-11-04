import Foundation

public enum ConversationSummarizer {
    /// Produce a concise 3–5 word summary title for a conversation.
    /// - Parameters:
    ///   - messages: Timeline messages (only .user and .assistant are considered).
    ///   - preferOnDeviceModel: Reserved for future use with FoundationModels. Currently ignored.
    /// - Returns: A short title suitable for a list row.
    public static func summarizeTitle(messages: [ACPMessage], preferOnDeviceModel: Bool = true) async -> String {
        // Try on-device Foundation Model first when available.
        #if canImport(FoundationModels)
        if #available(iOS 26.0, macOS 26.0, *), preferOnDeviceModel {
            let trimmed = messages.filter { $0.role == .user || $0.role == .assistant }
            if let fm = await FoundationModelSummarizer.trySummarizeTitle(messages: trimmed) {
                if !fm.isEmpty {
                    print("[Summary] used=foundation_model title=\(fm)")
                    return fm
                } else {
                    print("[Summary] fm_empty_fallback=true")
                }
            } else {
                print("[Summary] fm_unavailable_or_failed=true")
            }
        }
        #endif
        // Fallback: first user message (~5 words), skipping preface/system-like tags.
        let title = firstUserFiveWords(messages: messages)
        let out = title.isEmpty ? "Conversation" : title
        print("[Summary] used=first_user_5 title=\(out)")
        return out
    }

    static func heuristicTitle(from texts: [String]) -> String {
        // Prefer the first non-empty user message if available
        guard let seed = texts.first?.trimmingCharacters(in: .whitespacesAndNewlines), !seed.isEmpty else {
            return "Conversation"
        }
        // Take first sentence-ish chunk
        let sentence = seed.split(whereSeparator: { ".!?\n".contains($0) }).first.map(String.init) ?? seed
        // Tokenize and drop common stopwords/punctuation, cap to 5 words
        let stop: Set<String> = [
            "the","a","an","and","or","to","for","of","on","with","in","about","into","from","by","at","it","is","are","be","this","that","these","those","please","help","how","can","you","we","i","me","my"
        ]
        let words = sentence
            .lowercased()
            .replacingOccurrences(of: #"[^a-z0-9\s]"#, with: " ", options: .regularExpression)
            .split(separator: " ")
            .map(String.init)
            .filter { !$0.isEmpty && !stop.contains($0) }
        if words.isEmpty { return sentence.prefixWords(5).titleCased }
        let picked = Array(words.prefix(5)).joined(separator: " ")
        return picked.titleCased
    }

    static func firstUserFiveWords(messages: [ACPMessage]) -> String {
        let users = messages.filter { $0.role == .user }.sorted { $0.ts < $1.ts }
        guard let first = users.first else { return "" }
        let text = first.parts.compactMap { part -> String? in
            if case let .text(t) = part { return t.text } else { return nil }
        }.joined(separator: " ")
            .trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return "" }
        if isSystemPreface(text) { return "" }
        let words = text.split(whereSeparator: { $0.isWhitespace })
        return words.prefix(5).joined(separator: " ")
    }

    public static func isSystemPreface(_ text: String) -> Bool {
        let t = text.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        if t.hasPrefix("<user_instructions>") { return true }
        if t.hasPrefix("<environment_context>") { return true }
        if t.hasPrefix("<env_context>") { return true }
        if t.hasPrefix("<system>") { return true }
        if t.hasPrefix("<attachments>") { return true }
        return false
    }
}

// MARK: - Feature gate for on-device Foundation Models
fileprivate func foundationModelsAllowed() -> Bool {
    let env = ProcessInfo.processInfo.environment["OPENAGENTS_ENABLE_FM"] == "1"
    if env { return true }
    return UserDefaults.standard.bool(forKey: "enable_foundation_models")
}

fileprivate extension String { var nilIfEmpty: String? { self.isEmpty ? nil : self } }

fileprivate extension StringProtocol {
    func prefixWords(_ n: Int) -> String {
        let parts = self.split(separator: " ", omittingEmptySubsequences: true)
        return parts.prefix(n).joined(separator: " ")
    }
}

fileprivate extension String {
    var titleCased: String {
        return self.split(separator: " ").map { w in
            var s = String(w)
            if let first = s.unicodeScalars.first {
                s.replaceSubrange(s.startIndex...s.startIndex, with: String(Character(first)).uppercased())
            }
            return s
        }.joined(separator: " ")
    }
}
