// Firm reason: 6 (AppKit/Foundation process & network host for Pull Requests inbox, multi-forge source control, AI review tours, and Issues view; #13886).
import AppKit

private struct ForgePrRow {
    let number: Int32
    let title: String
    let author: String
    let headRef: String
    let baseRef: String
    let state: String
    let url: String
    let isDraft: Bool
    let isAuthored: Bool
    let reviewRequested: Bool
    let filesTotal: Int32
}

private struct ForgeIssueRow {
    let source: Int32 // 0 = GitHub, 1 = Linear
    let key: String
    let title: String
    let team: String
    let assignee: String
    let state: String
    let url: String
    let body: String
}

extension CoderWindow {
    func forgeAsked(_ what: Int32) {
        switch what {
        case 140: refreshForge()
        case 141: openForgePrInChat(tour: false)
        case 142: openForgePrInChat(tour: true)
        case 143: openForgeIssueInChat()
        case 144: persistForgePrViewed()
        default: break
        }
    }

    private func forgeViewedKey(repo: String, number: Int32) -> String {
        return "forge.viewed.\(repo).\(number)"
    }

    private func loadForgeViewed(repo: String, number: Int32) -> (UInt64, Int32) {
        let key = forgeViewedKey(repo: repo, number: number)
        guard let dict = UserDefaults.standard.dictionary(forKey: key) else { return (0, 0) }
        let mask = (dict["mask"] as? NSNumber)?.uint64Value ?? 0
        let count = (dict["count"] as? NSNumber)?.int32Value ?? 0
        return (mask, count)
    }

    private func persistForgePrViewed() {
        let num = cd_forge_selected_pr(page)
        guard num > 0 else { return }
        var mask: UInt64 = 0
        var count: Int32 = 0
        guard cd_forge_selected_pr_viewed(page, &mask, &count) != 0 else { return }
        let repo = folder.standardizedFileURL.path
        let key = forgeViewedKey(repo: repo, number: num)
        UserDefaults.standard.set(["mask": NSNumber(value: mask), "count": NSNumber(value: count)], forKey: key)
    }

    private func openForgePrInChat(tour: Bool) {
        var buf = [CChar](repeating: 0, count: 8192)
        let n = cd_forge_prompt(page, tour ? 1 : 0, &buf, Int32(buf.count))
        guard n > 0 else { return }
        let prompt = String(cString: buf)
        spawnForgeWorktreeChat(prompt: prompt)
    }

    private func openForgeIssueInChat() {
        var buf = [CChar](repeating: 0, count: 8192)
        let n = cd_forge_issue_prompt(page, &buf, Int32(buf.count))
        guard n > 0 else { return }
        let prompt = String(cString: buf)
        spawnForgeWorktreeChat(prompt: prompt)
    }

    /// Spawns a new chat in a fresh git worktree pre-seeded with the PR, AI Review Tour, or Issue prompt.
    private func spawnForgeWorktreeChat(prompt: String) {
        cd_forge_close(page)
        newChat()
        let nextAct = cd_action(page)
        if nextAct != 0 { act(nextAct) }
        if cd_set_text(page, prompt) != 0 {
            _ = cd_submit(page)
            let sendAct = cd_action(page)
            if sendAct != 0 { act(sendAct) }
        }
        focusComposer()
        wake()
    }

    func refreshForge() {
        let cwd = folder.standardizedFileURL.path
        cd_forge_set_status(page, 1, "Refreshing pull requests and issues…")
        wake()

        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            let remoteURL = CoderWindow.runTool(cwd: cwd, exe: "/usr/bin/git", args: ["remote", "get-url", "origin"])?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
            let ghPath = CoderWindow.resolveExecutable("gh")
            var prs: [ForgePrRow] = []
            var issues: [ForgeIssueRow] = []
            var notes: [String] = []

            if let gh = ghPath, !remoteURL.isEmpty {
                let me = (CoderWindow.runTool(cwd: cwd, exe: gh, args: ["api", "user", "--jq", ".login"]) ?? "")
                    .trimmingCharacters(in: .whitespacesAndNewlines).lowercased()

                if let prJson = CoderWindow.runTool(cwd: cwd, exe: gh, args: [
                    "pr", "list", "--limit", "50",
                    "--json", "number,title,author,headRefName,baseRefName,state,url,isDraft,reviewRequests,files"
                ]), let data = prJson.data(using: .utf8),
                   let arr = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]] {
                    for obj in arr {
                        let number = Int32(obj["number"] as? Int ?? 0)
                        guard number > 0 else { continue }
                        let title = obj["title"] as? String ?? ""
                        let author = (obj["author"] as? [String: Any])?["login"] as? String ?? ""
                        let headRef = obj["headRefName"] as? String ?? ""
                        let baseRef = obj["baseRefName"] as? String ?? ""
                        let state = obj["state"] as? String ?? "OPEN"
                        let url = obj["url"] as? String ?? ""
                        let isDraft = obj["isDraft"] as? Bool ?? false
                        let files = obj["files"] as? [[String: Any]] ?? []
                        let reviewReqs = obj["reviewRequests"] as? [[String: Any]] ?? []
                        var revRequested = false
                        for req in reviewReqs {
                            if let login = (req["login"] as? String)?.lowercased(), !me.isEmpty, login == me {
                                revRequested = true
                                break
                            }
                        }
                        let isAuthored = !me.isEmpty && author.lowercased() == me
                        prs.append(ForgePrRow(
                            number: number, title: title, author: author,
                            headRef: headRef, baseRef: baseRef, state: state,
                            url: url, isDraft: isDraft, isAuthored: isAuthored,
                            reviewRequested: revRequested, filesTotal: Int32(files.count)
                        ))
                    }
                } else {
                    notes.append("GitHub PRs unavailable (check gh auth)")
                }

                if let issJson = CoderWindow.runTool(cwd: cwd, exe: gh, args: [
                    "issue", "list", "--limit", "50",
                    "--json", "number,title,labels,assignees,state,url,body"
                ]), let data = issJson.data(using: .utf8),
                   let arr = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]] {
                    for obj in arr {
                        let number = obj["number"] as? Int ?? 0
                        guard number > 0 else { continue }
                        let title = obj["title"] as? String ?? ""
                        let labels = obj["labels"] as? [[String: Any]] ?? []
                        let team = (labels.first?["name"] as? String) ?? "GitHub"
                        let assignees = obj["assignees"] as? [[String: Any]] ?? []
                        let assignee = (assignees.first?["login"] as? String) ?? ""
                        let state = obj["state"] as? String ?? "OPEN"
                        let url = obj["url"] as? String ?? ""
                        let bodyRaw = (obj["body"] as? String ?? "").replacingOccurrences(of: "\r\n", with: " ").replacingOccurrences(of: "\n", with: " ")
                        let body = String(bodyRaw.prefix(240))
                        issues.append(ForgeIssueRow(
                            source: 0, key: "#\(number)", title: title, team: team,
                            assignee: assignee, state: state, url: url, body: body
                        ))
                    }
                }
            } else if remoteURL.isEmpty {
                notes.append("No git origin remote detected in this folder")
            } else {
                notes.append("Install GitHub CLI (gh) for live PR & Issue sync")
            }

            // Optional Linear multi-team issue fetch if LINEAR_API_KEY is configured
            let linearKey = ProcessInfo.processInfo.environment["LINEAR_API_KEY"]
                ?? UserDefaults.standard.string(forKey: "linearApiKey")
                ?? ""
            if !linearKey.isEmpty {
                let linearIssues = CoderWindow.fetchLinearIssues(apiKey: linearKey)
                issues.append(contentsOf: linearIssues)
            }

            DispatchQueue.main.async { [weak self] in
                guard let self else { return }
                let kind = cd_forge_set_remote(self.page, remoteURL)
                cd_forge_pr_clear(self.page)
                for pr in prs {
                    let (mask, cnt) = self.loadForgeViewed(repo: cwd, number: pr.number)
                    _ = cd_forge_pr_add(
                        self.page, pr.number, pr.title, pr.author, pr.headRef, pr.baseRef,
                        pr.state, pr.url, pr.isDraft ? 1 : 0, pr.isAuthored ? 1 : 0,
                        pr.reviewRequested ? 1 : 0, pr.filesTotal, mask, cnt
                    )
                }
                cd_forge_pr_finalize(self.page)

                cd_forge_issue_clear(self.page)
                for iss in issues {
                    _ = cd_forge_issue_add(
                        self.page, iss.source, iss.key, iss.title, iss.team,
                        iss.assignee, iss.state, iss.url, iss.body
                    )
                }

                let statusMsg: String
                if !notes.isEmpty && prs.isEmpty && issues.isEmpty {
                    statusMsg = notes.joined(separator: " · ")
                } else if kind > 1 && prs.isEmpty {
                    statusMsg = "Detected remote forge; connect CLI or token for live sync"
                } else {
                    statusMsg = "\(prs.count) PRs · \(issues.count) issues"
                }
                cd_forge_set_status(self.page, 0, statusMsg)
                self.wake()
            }
        }
    }

    private static func resolveExecutable(_ name: String) -> String? {
        for dir in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"] {
            let p = dir + "/" + name
            if FileManager.default.isExecutableFile(atPath: p) { return p }
        }
        return nil
    }

    private static func runTool(cwd: String, exe: String, args: [String]) -> String? {
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: exe)
        proc.arguments = args
        proc.currentDirectoryURL = URL(fileURLWithPath: cwd, isDirectory: true)
        let out = Pipe()
        proc.standardOutput = out
        proc.standardError = Pipe()
        do {
            try proc.run()
            let data = out.fileHandleForReading.readDataToEndOfFile()
            proc.waitUntilExit()
            guard proc.terminationStatus == 0 else { return nil }
            return String(data: data, encoding: .utf8)
        } catch {
            return nil
        }
    }

    private static func fetchLinearIssues(apiKey: String) -> [ForgeIssueRow] {
        guard let url = URL(string: "https://api.linear.app/graphql") else { return [] }
        var req = URLRequest(url: url, timeoutInterval: 10)
        req.httpMethod = "POST"
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.setValue(apiKey, forHTTPHeaderField: "Authorization")
        let gql = """
        {"query":"{ issues(first: 40, filter: { state: { type: { nin: [\\"completed\\", \\"canceled\\"] } } }) { nodes { identifier title url description state { name } team { key name } assignee { name } } } }"}
        """
        req.httpBody = gql.data(using: .utf8)
        let sem = DispatchSemaphore(value: 0)
        var result: [ForgeIssueRow] = []
        URLSession.shared.dataTask(with: req) { data, _, _ in
            defer { sem.signal() }
            guard let data,
                  let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let d = root["data"] as? [String: Any],
                  let iss = d["issues"] as? [String: Any],
                  let nodes = iss["nodes"] as? [[String: Any]] else { return }
            for n in nodes {
                let key = n["identifier"] as? String ?? ""
                guard !key.isEmpty else { continue }
                let title = n["title"] as? String ?? ""
                let urlStr = n["url"] as? String ?? ""
                let state = (n["state"] as? [String: Any])?["name"] as? String ?? "Open"
                let teamObj = n["team"] as? [String: Any]
                let team = (teamObj?["key"] as? String) ?? (teamObj?["name"] as? String) ?? "Linear"
                let assignee = (n["assignee"] as? [String: Any])?["name"] as? String ?? ""
                let descRaw = (n["description"] as? String ?? "").replacingOccurrences(of: "\r\n", with: " ").replacingOccurrences(of: "\n", with: " ")
                let body = String(descRaw.prefix(240))
                result.append(ForgeIssueRow(
                    source: 1, key: key, title: title, team: team,
                    assignee: assignee, state: state, url: urlStr, body: body
                ))
            }
        }.resume()
        _ = sem.wait(timeout: .now() + 10)
        return result
    }
}
