// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
// Live Git diff viewer and inline review comments (page/gitdiff.coax, GitHub #13870): runs real
// `git status --porcelain` + `git diff` (staged, unstaged, untracked) in the active chat's folder/worktree
// and presents the Coax overlay dialog for adding/editing inline review comments.
import AppKit

extension CoderWindow {
    /// Refreshes the live working-tree git diff for the active chat's folder/worktree on a background queue.
    func refreshGitDiff() {
        let dir = folder
        let maxBytes = Int(cd_git_diff_max())
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            let payload = CoderWindow.collectGitDiff(in: dir, maxBytes: maxBytes)
            DispatchQueue.main.async {
                guard let self, self.folder == dir else { return }
                let data = Data(payload.utf8)
                data.withUnsafeBytes { raw in
                    let ptr = raw.bindMemory(to: CChar.self).baseAddress
                    cd_git_diff_set(self.page, ptr, Int32(data.count))
                }
                self.wake()
            }
        }
    }

    /// Runs git in `dir` and returns stdout if exit status is 0.
    private static func runGit(_ args: [String], in dir: URL) -> (status: Int32, out: String) {
        let p = Process()
        p.executableURL = URL(fileURLWithPath: "/usr/bin/git")
        p.arguments = args
        p.currentDirectoryURL = dir
        let pipe = Pipe()
        p.standardOutput = pipe
        p.standardError = FileHandle.nullDevice
        do {
            try p.run()
        } catch {
            return (-1, "")
        }
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        p.waitUntilExit()
        return (p.terminationStatus, String(data: data, encoding: .utf8) ?? "")
    }

    /// Collects `# status:` lines from `git status --porcelain` plus unified `git diff` (staged + unstaged + untracked).
    static func collectGitDiff(in dir: URL, maxBytes: Int) -> String {
        let top = runGit(["rev-parse", "--show-toplevel"], in: dir)
        guard top.status == 0 else { return "" }
        let rootPath = top.out.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !rootPath.isEmpty else { return "" }
        let root = URL(fileURLWithPath: rootPath)

        let st = runGit(["status", "--porcelain=v1", "-uall"], in: root)
        guard st.status == 0 else { return "" }

        var out = ""
        out.reserveCapacity(min(maxBytes, 65536))
        var untracked: [String] = []
        for rawLine in st.out.split(separator: "\n", omittingEmptySubsequences: true) {
            let line = String(rawLine)
            guard line.count >= 4 else { continue }
            out += "# status: " + line + "\n"
            if line.hasPrefix("?? ") {
                var path = String(line.dropFirst(3))
                if path.hasPrefix("\"") && path.hasSuffix("\"") && path.count >= 2 {
                    path = String(path.dropFirst().dropLast())
                }
                untracked.append(path)
            }
        }
        if out.isEmpty { return "" }

        let hasHead = runGit(["rev-parse", "--verify", "HEAD"], in: root).status == 0
        if hasHead {
            let diff = runGit(["diff", "--no-color", "--no-ext-diff", "HEAD"], in: root)
            if diff.status == 0 && !diff.out.isEmpty {
                out += diff.out
                if !out.hasSuffix("\n") { out += "\n" }
            }
        } else {
            let cached = runGit(["diff", "--no-color", "--no-ext-diff", "--cached"], in: root)
            if cached.status == 0 && !cached.out.isEmpty {
                out += cached.out
                if !out.hasSuffix("\n") { out += "\n" }
            }
            let unstaged = runGit(["diff", "--no-color", "--no-ext-diff"], in: root)
            if unstaged.status == 0 && !unstaged.out.isEmpty {
                out += unstaged.out
                if !out.hasSuffix("\n") { out += "\n" }
            }
        }

        // Synthesize unified diffs for untracked UTF-8 text files (bounded per file and total).
        let perFileMax = 65536
        for rel in untracked {
            if out.utf8.count >= maxBytes - 512 { break }
            let url = root.appendingPathComponent(rel)
            var isDir: ObjCBool = false
            guard FileManager.default.fileExists(atPath: url.path, isDirectory: &isDir), !isDir.boolValue else { continue }
            let size = ((try? FileManager.default.attributesOfItem(atPath: url.path))?[.size] as? NSNumber)?.intValue ?? 0
            if size <= 0 || size > perFileMax { continue }
            guard let data = try? Data(contentsOf: url), !data.contains(0),
                  let text = String(data: data, encoding: .utf8) else { continue }
            let lines = text.split(separator: "\n", omittingEmptySubsequences: false)
            let count = lines.last == "" && lines.count > 1 ? lines.count - 1 : lines.count
            guard count > 0 else { continue }
            out += "diff --git a/\(rel) b/\(rel)\nnew file mode 100644\n--- /dev/null\n+++ b/\(rel)\n@@ -0,0 +1,\(count) @@\n"
            for i in 0..<count {
                out += "+" + String(lines[i]) + "\n"
                if out.utf8.count >= maxBytes - 64 { break }
            }
        }
        return out
    }

    /// Presents the inline review comment dialog for the line or comment picked in the Changes pane (CP_ASK_REVIEW_COMMENT).
    func openReviewCommentDialog() {
        var pathBuf = [CChar](repeating: 0, count: 512)
        var snipBuf = [CChar](repeating: 0, count: 256)
        var bodyBuf = [CChar](repeating: 0, count: 1024)
        var meta = [Int32](repeating: 0, count: 4)
        let pn = cd_review_pick(page, &pathBuf, Int32(pathBuf.count), &snipBuf, Int32(snipBuf.count), &bodyBuf, Int32(bodyBuf.count), &meta)
        guard pn > 0 else { return }
        let path = String(cString: pathBuf)
        let snippet = String(cString: snipBuf)
        let existing = String(cString: bodyBuf)
        let editId = meta[0]
        let startLine = meta[1]
        let endLine = meta[2]
        let side = meta[3]
        let sideNote = side == 1 ? " (removed)" : ""
        let rangeStr = endLine > startLine ? "\(startLine)-\(endLine)" : "\(startLine)"
        let subtitle = snippet.isEmpty ? "`\(path):\(rangeStr)`\(sideNote)" : "`\(path):\(rangeStr)`\(sideNote)\n\(snippet)"

        if editId > 0 {
            var d = CoderDialog(size: .md, title: "Edit Review Comment", body: subtitle)
            d.fields = [
                CoderDialog.Field(label: "Comment", value: existing, hint: "Write your review comment…", lines: 4)
            ]
            d.buttons = [
                CoderDialog.Button(title: "Delete", kind: .danger, aside: true),
                CoderDialog.Button(title: "Cancel"),
                CoderDialog.Button(title: "Save", kind: .primary)
            ]
            d.confirm = 2; d.cancel = 1; d.focus = 0
            present(d) { [weak self] answer, values in
                guard let self else { return }
                if answer == 0 {
                    _ = cd_review_comment_delete(self.page, editId)
                } else if answer == 2, let text = values.first {
                    _ = cd_review_comment_edit(self.page, editId, text)
                }
                self.wake()
            }
        } else {
            var d = CoderDialog(size: .md, title: "Add Review Comment", body: subtitle)
            d.fields = [
                CoderDialog.Field(label: "Comment", value: "", hint: "Write your review comment…", lines: 4),
                CoderDialog.Field(label: "End line (optional, for range)", value: "\(endLine)", hint: "\(startLine)", lines: 1)
            ]
            d.buttons = [
                CoderDialog.Button(title: "Cancel"),
                CoderDialog.Button(title: "Add Comment", kind: .primary)
            ]
            d.confirm = 1; d.cancel = 0; d.focus = 0
            present(d) { [weak self] answer, values in
                guard let self, answer == 1, let text = values.first else { return }
                let parsedEnd = values.count > 1 ? (Int32(values[1].trimmingCharacters(in: .whitespacesAndNewlines)) ?? endLine) : endLine
                let finalEnd = max(startLine, parsedEnd)
                _ = cd_review_comment_add(self.page, path, startLine, finalEnd, side, snippet, text)
                self.wake()
            }
        }
    }
}
