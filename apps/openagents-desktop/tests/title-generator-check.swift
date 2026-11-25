// Firm reason: 6 (standalone Swift verification check for TitleGenerator.swift: T3-parity prompts,
// JSON/plain-text sanitization, Vertex AI URL/region discovery, credit-exhaustion cooldown & fallback).
import Foundation

func check(_ cond: @autoclosure () -> Bool, _ msg: String) {
    if !cond() {
        fputs("FAIL: \(msg)\n", stderr)
        exit(1)
    }
}

@main
struct TitleGeneratorCheck {
    static func main() {
        // 1. Prompt builder: initial prompt with project exclusion & editorial rules
        let p1 = ThreadTitlePrompts.buildInitialPrompt(
            message: "Fix the sidebar scroll jump when opening a long chat",
            projectTitle: "openagents"
        )
        check(p1.contains("Return JSON with keys title and needsRefinement."), "initial prompt asks for JSON")
        check(p1.contains("3-8 words, fewer than 40 characters."), "initial prompt includes length rule")
        check(p1.contains("Never include the project name (\"openagents\")"), "initial prompt excludes project name")
        check(p1.contains("User message:\nFix the sidebar scroll jump"), "initial prompt includes user message")

        // 2. Prompt builder: fork source section
        let fork = ThreadTitleForkSource(title: "Fix Sidebar Scroll", history: "USER:\nFix sidebar scroll\n\nASSISTANT:\nFound the cause.")
        let pFork = ThreadTitlePrompts.buildInitialPrompt(
            message: "Now let's also virtualize the file tree",
            projectTitle: "openagents",
            forkSource: fork
        )
        check(pFork.contains("This chat is a fork of the chat \"Fix Sidebar Scroll\"."), "fork prompt names source")
        check(pFork.contains("User message after the fork:\nNow let's also virtualize the file tree"), "fork prompt labels post-fork message")

        // 3. Prompt builder: regenerate prompt preserves tail & previous title
        let longTranscript = String(repeating: "a", count: 9_000) + "FINAL_GOAL_MARKER"
        let pRegen = ThreadTitlePrompts.buildRegeneratePrompt(
            previousTitle: "Fix this bug",
            transcript: longTranscript,
            projectTitle: "openagents"
        )
        check(pRegen.contains("The previous title was \"Fix this bug\"."), "regenerate prompt includes previous title")
        check(pRegen.contains("[Earlier content truncated]\n\n"), "regenerate prompt marks head truncation")
        check(pRegen.contains("FINAL_GOAL_MARKER"), "regenerate prompt preserves tail")

        // 4. Conversation JSON extraction
        let chatJSON = """
        {
          "v": 1,
          "model": "flawed:gemini-4-neon",
          "turns": [
            {"ask": "Fix this", "reply": "I found a null pointer in the sidebar renderer."},
            {"ask": "Great, patch it and add a test", "reply": "Patched sidebar.coax and verified."}
          ]
        }
        """
        let extracted = ThreadTitlePrompts.extractConversation(fromChatJSON: chatJSON)
        check(extracted != nil, "extracted conversation")
        check(extracted?.firstAsk == "Fix this", "firstAsk")
        check(extracted?.lastAsk == "Great, patch it and add a test", "lastAsk")
        check(extracted?.turnCount == 2, "turnCount")
        check(extracted?.transcript.contains("USER:\nFix this") == true, "transcript has user turn")
        check(extracted?.transcript.contains("ASSISTANT:\nI found a null pointer") == true, "transcript has assistant turn")

        // 5. Response parsing & sanitization (T3 parity)
        let r1 = ThreadTitleSanitizer.parseResponse("{\"title\":\"Sidebar Scroll Jump Fix\",\"needsRefinement\":false}", backend: "vertex")
        check(r1?.title == "Sidebar Scroll Jump Fix" && r1?.needsRefinement == false, "strict JSON parse")

        let r2 = ThreadTitleSanitizer.parseResponse("```json\n{\"title\": \"  \\\"Investigate Vague Bug.\\\" \", \"needsRefinement\": true}\n```", backend: "vertex")
        check(r2?.title == "Investigate Vague Bug" && r2?.needsRefinement == true, "fenced JSON + quote/period strip")

        let r3 = ThreadTitleSanitizer.parseResponse("Here is the JSON:\n{\"title\":\"Refactor Auth Token Cache\",\"needsRefinement\":false}\nDone.", backend: "local")
        check(r3?.title == "Refactor Auth Token Cache" && r3?.needsRefinement == false, "embedded JSON extraction")

        let r4 = ThreadTitleSanitizer.parseResponse("Title: \"Improve Chat Title Generation.\"\nExtra line ignored", backend: "local")
        check(r4?.title == "Improve Chat Title Generation" && r4?.needsRefinement == false, "plain text fallback strips label, quotes, period, second line")

        let longTitle = String(repeating: "x", count: 150)
        let sanitizedLong = ThreadTitleSanitizer.sanitize(longTitle)
        check(sanitizedLong.count == 120 && sanitizedLong.hasSuffix("..."), "long title truncated to 120 chars")

        check(ThreadTitleSanitizer.sanitize("   \n\t  ") == "", "blank sanitizes to empty")

        // 6. Vertex AI host & URL mapping (matches coax/lib/gateway/vertex.coax)
        check(ChatTitleGenerator.vertexHost(forLocation: "us") == "https://aiplatform.us.rep.googleapis.com", "us multi-region host")
        check(ChatTitleGenerator.vertexHost(forLocation: "eu") == "https://aiplatform.eu.rep.googleapis.com", "eu multi-region host")
        check(ChatTitleGenerator.vertexHost(forLocation: "global") == "https://aiplatform.googleapis.com", "global host")
        check(ChatTitleGenerator.vertexHost(forLocation: "us-central1") == "https://us-central1-aiplatform.googleapis.com", "regional host")
        let vURL = ChatTitleGenerator.vertexGenerateURL(project: "openagentsgemini", location: "us", model: "google/gemini-4-hydrogen")
        check(vURL?.absoluteString == "https://aiplatform.us.rep.googleapis.com/v1/projects/openagentsgemini/locations/us/publishers/google/models/gemini-4-hydrogen:generateContent", "vertex native generateContent URL")

        // 7. Gemini native response parsing (skips thought parts)
        let geminiResp = """
        {
          "candidates": [
            {
              "content": {
                "parts": [
                  {"thought": true, "text": "thinking about the title..."},
                  {"text": "{\\"title\\":\\"Add Conversation Title Generation\\",\\"needsRefinement\\":false}"}
                ]
              }
            }
          ]
        }
        """.data(using: .utf8)!
        let geminiText = ChatTitleGenerator.extractGeminiNativeText(from: geminiResp)
        check(geminiText == "{\"title\":\"Add Conversation Title Generation\",\"needsRefinement\":false}", "gemini native skips thought parts")

        // 8. End-to-end backend fallback & credit-exhaustion cooldown with mock transport
        var seenHosts: [String] = []
        let gen = ChatTitleGenerator { req in
            let url = req.url?.absoluteString ?? ""
            seenHosts.append(url)
            if url.contains("aiplatform") {
                // Simulate credits exhausted / quota exceeded on Vertex AI (HTTP 429)
                return (nil, 429)
            }
            if url.contains("generativelanguage.googleapis.com") {
                // Simulate Gemini API key unavailable / 403
                return (nil, 403)
            }
            if url.contains("127.0.0.1:18080/v1/chat/completions") {
                let body = """
                {"choices":[{"message":{"content":"{\\"title\\":\\"Offline Metal Fallback Title\\",\\"needsRefinement\\":false}"}}]}
                """.data(using: .utf8)!
                return (body, 200)
            }
            return (nil, 404)
        }
        gen.localServerProvider = { ("http://127.0.0.1:18080", "inkling-2-mini") }
        let fallbackResult = gen.runPrompt("Test prompt")
        check(fallbackResult?.title == "Offline Metal Fallback Title", "falls back cleanly to local Psionic when cloud credits exhausted")
        check(fallbackResult?.backend == "local:inkling-2-mini", "records winning backend")

        // Second call should skip cooled-down Vertex/Gemini backends and go straight to local!
        seenHosts.removeAll()
        let secondResult = gen.runPrompt("Test prompt 2")
        check(secondResult?.title == "Offline Metal Fallback Title", "second call succeeds via local")
        check(!seenHosts.contains(where: { $0.contains("aiplatform") }), "Vertex AI cooled down after 429")
        check(!seenHosts.contains(where: { $0.contains("generativelanguage.googleapis.com") }), "Gemini API cooled down after 403")

        // 9. Project title derivation
        check(ThreadTitlePrompts.projectTitle(fromFolder: "/Users/test/work/openagents") == "openagents", "project basename")
        check(ThreadTitlePrompts.projectTitle(fromFolder: "/") == nil, "root omitted")
        check(ThreadTitlePrompts.projectTitle(fromFolder: FileManager.default.homeDirectoryForCurrentUser.path) == nil, "home dir omitted")

        print("title-generator-check: ok")
    }
}
