import Foundation

public enum ConversationSummarizer {
    /// Produce a concise 3–5 word summary title for a conversation.
    /// - Parameters:
    ///   - messages: Timeline messages (only .user and .assistant are considered).
    ///   - preferOnDeviceModel: Reserved for future use with FoundationModels. Currently ignored.
    /// - Returns: A short title suitable for a list row.
    public static func summarizeTitle(messages: [ACPMessage], preferOnDeviceModel: Bool = true) async -> String {
        // Filter to user/assistant, keep chronological order, take last ~10
        var seq = messages.filter { $0.role == .user || $0.role == .assistant }
        seq.sort { $0.ts < $1.ts }
        if seq.count > 10 { seq = Array(seq.suffix(10)) }
        // Extract plain text content
        let texts: [String] = seq.compactMap { m in
            let s = m.parts.compactMap { part -> String? in
                if case let .text(t) = part { return t.text } else { return nil }
            }.joined(separator: "\n")
            return s.nilIfEmpty
        }

        // Try on-device foundation model if available/allowed.
        if preferOnDeviceModel && foundationModelsAllowed() {
            #if canImport(FoundationModels)
            if #available(iOS 26.0, macOS 26.0, *), let fm = await FoundationModelSummarizer.trySummarizeTitle(messages: seq) {
                if !fm.isEmpty { return fm }
            }
            #endif
        }

        // Heuristic fallback until FoundationModels is wired.
        let candidate = heuristicTitle(from: texts)
        return candidate
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
