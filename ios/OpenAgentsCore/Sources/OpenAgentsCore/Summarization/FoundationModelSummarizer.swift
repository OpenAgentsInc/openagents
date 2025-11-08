import Foundation

#if canImport(FoundationModels)
import FoundationModels

@available(iOS 26.0, macOS 26.0, * )
enum FoundationModelSummarizer {
    /// Attempt to summarize the conversation title using Apple's on-device foundation model.
    /// Returns nil if summarization fails so callers can fall back.
    @available(iOS 26.0, macOS 26.0, * )
    static func trySummarizeTitle(messages: [ACPMessage]) async -> String? {
        // Keep the last ~10 user/assistant messages as plain text lines
        var seq = messages.filter { $0.role == .user || $0.role == .assistant }
        seq.sort { $0.ts < $1.ts }
        if seq.count > 10 { seq = Array(seq.suffix(10)) }
        let lines: [String] = seq.compactMap { m in
            let speaker = (m.role == .user) ? "User" : "Assistant"
            let text = m.parts.compactMap { part -> String? in
                if case let .text(t) = part { return t.text } else { return nil }
            }.joined(separator: " ")
            return text.isEmpty ? nil : "\(speaker): \(text)"
        }
        guard !lines.isEmpty else { return nil }

        let model = SystemLanguageModel.default
        switch model.availability {
        case .available:
            break
        case .unavailable(let reason):
            OpenAgentsLog.app.debug("FM unavailable reason=\(String(describing: reason))")
            return nil
        }

        let instructions = Instructions("""
        You create very short, descriptive conversation titles.
        Output only 3-5 plain words, no punctuation, no quotes.
        """)

        let session = LanguageModelSession(model: model, tools: [], instructions: instructions)
        try? session.prewarm(promptPrefix: nil)

        let prompt = lines.joined(separator: "\n") + "\n\nTitle (3-5 words):"
        do {
            let options = GenerationOptions(temperature: 0.1)
            let resp = try await session.respond(to: prompt, options: options)
            // Avoid KVC; use description parsing to be resilient across SDKs.
            let desc = String(describing: resp)
            if let extracted = extractFromDescription(desc), !extracted.isEmpty {
                let cleaned = cleanup(extracted)
                return cleaned.isEmpty ? nil : cleaned
            }
            return nil
        } catch {
            return nil
        }
    }

    private static func cleanup(_ s: String) -> String {
        let trimmed = s.trimmingCharacters(in: .whitespacesAndNewlines)
            .replacingOccurrences(of: #"["'.,;:()\[\]{}]+"#, with: "", options: .regularExpression)
        // Cap to 5 words
        let words = trimmed.split(whereSeparator: { $0.isWhitespace })
        return words.prefix(5).joined(separator: " ")
    }

    // Parse Response<String> description to extract content= or rawContent=
    static func extractFromDescription(_ desc: String) -> String? {
        // Guardrail messages → treat as failure
        if desc.localizedCaseInsensitiveContains("Safety guardrails were triggered") { return nil }
        if desc.localizedCaseInsensitiveContains("Error during development") { return nil }
        // Try to capture content: '...'
        if let m = match(desc, pattern: #"content:\s*'([^']*)'"#) { return m }
        if let m = match(desc, pattern: #"content:\s*\"([^\"]*)\""#) { return m }
        if let m = match(desc, pattern: #"rawContent:\s*'([^']*)'"#) { return m }
        if let m = match(desc, pattern: #"rawContent:\s*\"([^\"]*)\""#) { return m }
        return nil
    }

    private static func match(_ s: String, pattern: String) -> String? {
        do {
            let re = try NSRegularExpression(pattern: pattern, options: [])
            let range = NSRange(s.startIndex..<s.endIndex, in: s)
            if let m = re.firstMatch(in: s, options: [], range: range), m.numberOfRanges >= 2,
               let r = Range(m.range(at: 1), in: s) {
                return String(s[r])
            }
        } catch {}
        return nil
    }
}
#endif

// MARK: - Test-only shim to reach private helper without exposing it publicly
func FoundationModelSummarizer_extractFromDescription(_ desc: String) -> String? {
    #if canImport(FoundationModels)
    if #available(iOS 26.0, macOS 26.0, *) {
        return FoundationModelSummarizer.extractFromDescription(desc)
    }
    #endif
    // Fallback: basic parse for tests even without FoundationModels present
    // Duplicate the minimal logic
    if desc.localizedCaseInsensitiveContains("Safety guardrails were triggered") { return nil }
    if desc.localizedCaseInsensitiveContains("Error during development") { return nil }
    func match(_ s: String, _ pattern: String) -> String? {
        do {
            let re = try NSRegularExpression(pattern: pattern, options: [])
            let range = NSRange(s.startIndex..<s.endIndex, in: s)
            if let m = re.firstMatch(in: s, options: [], range: range), m.numberOfRanges >= 2,
               let r = Range(m.range(at: 1), in: s) {
                return String(s[r])
            }
        } catch {}
        return nil
    }
    return match(desc, #"content:\s*'([^']*)'"#)
        ?? match(desc, #"content:\s*\"([^\"]*)\""#)
        ?? match(desc, #"rawContent:\s*'([^']*)'"#)
        ?? match(desc, #"rawContent:\s*\"([^\"]*)\""#)
}
