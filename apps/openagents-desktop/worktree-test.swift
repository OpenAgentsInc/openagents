// Firm reason: 6 (checks the Swift worktree code in Worktree.swift and Store.swift; no Coax runs it).
// Checks each chat's own worktree (GitHub #13758): names, making one from a repository and cloning
// what it lists, finding it again, keeping it with the chat, and never removing one with changes.
//   apps/coder-desktop/worktree-test.sh
import Foundation

var failed = 0
func check(_ ok: Bool, _ what: String, line: Int = #line) {
    print((ok ? "ok   " : "FAIL ") + what)
    if !ok { failed += 1; print("     at line \(line)") }
}
func sh(_ cmd: String, in dir: String) -> (Int32, String) { ChatWorktree.run("/bin/sh", ["-c", "cd '\(dir)' && " + cmd]) }

@main
struct WorktreeTest {
    static func main() {
        let tmp = ProcessInfo.processInfo.environment["WT_TEST_DIR"] ?? NSTemporaryDirectory()
        let fm = FileManager.default
        let repo = tmp + "/work/my repo", root = tmp + "/home/.openagents/worktrees"
        let id = "3f2a9c1e-0b7d-4e55-9a1b-2c3d4e5f6a7b"

        // names
        let slug = ChatWorktree.slug(repo)
        check(slug.hasPrefix("my-repo-") && slug.count == 16, "a repository's folder is its name and 8 hex digits: \(slug)")
        check(ChatWorktree.slug("/a/x/openagents") != ChatWorktree.slug("/b/openagents"), "two repositories with one name get two folders")
        check(ChatWorktree.slug("/w/.Hidden Repo!") .hasPrefix("hidden-repo-"), "a repository's name keeps letters, digits, dots and dashes")
        // one repository reached two ways gets one name: /tmp and /private/tmp, and a linked folder (#13771)
        let viaTmp = ChatWorktree.run("/usr/bin/mktemp", ["-d", "/tmp/wt-real.XXXXXX"]).output
        check(viaTmp.hasPrefix("/tmp/") && ChatWorktree.slug(viaTmp) == ChatWorktree.slug("/private" + viaTmp), "/tmp/X and /private/tmp/X name one folder: \(ChatWorktree.slug(viaTmp))")
        check(ChatWorktree.real(viaTmp) == "/private" + viaTmp, "the real path of /tmp/X is /private/tmp/X")
        try? fm.removeItem(atPath: viaTmp)
        check(ChatWorktree.branch(id) == "coder/" + id && ChatWorktree.branch("../x Y") == "coder/xy", "the branch is coder/ and the chat id, cleaned")
        check(ChatWorktree.path(repo: repo, chat: id, root: root) == root + "/" + slug + "/" + id, "the worktree is ROOT/REPO/CHAT")
        check(ChatWorktree.shown("coder/" + id) == "coder/3f2a9c1e" && ChatWorktree.shown("main") == "main", "the composer shows coder/ and 8 characters of the id")

        // a repository with an ignored tools folder its worktree.json lists, and a link back into it
        _ = sh("mkdir -p 'my repo/tools/bin' 'my repo/.coder' 'my repo/sub'", in: tmp + "/work")
        try? "tools/bin/\n".write(toFile: repo + "/.gitignore", atomically: true, encoding: .utf8)
        try? "{\"clone\":[\"tools/bin\",\"../escape\",\"/abs\",\"missing\"]}".write(toFile: repo + "/.coder/worktree.json", atomically: true, encoding: .utf8)
        try? "compiler v1".write(toFile: repo + "/tools/bin/coax", atomically: true, encoding: .utf8)
        try? "one\n".write(toFile: repo + "/a.txt", atomically: true, encoding: .utf8)
        try? fm.createSymbolicLink(atPath: repo + "/tools/bin/back", withDestinationPath: repo + "/tools/bin")
        try? fm.createSymbolicLink(atPath: repo + "/tools/bin/rel", withDestinationPath: "coax")
        let g = "git -c user.name=t -c user.email=t@example.invalid"
        let (ini, iniOut) = sh("git init -q -b main . && \(g) add -A && \(g) commit -qm start", in: repo)
        check(ini == 0, "a scratch repository: \(iniOut)")
        check(ChatWorktree.mainCheckout(of: repo + "/sub") == repo && ChatWorktree.mainCheckout(of: tmp) == nil, "a folder in a main checkout finds it; one outside finds none")
        check(ChatWorktree.clonePaths(repo: repo) == ["tools/bin", "missing"], "worktree.json's clone list keeps paths inside the repository")

        // made: on its branch, from HEAD (no origin/main here), with the tools cloned in and the link back cut
        guard case .success(let w) = ChatWorktree.create(repo: repo, chat: id, root: root) else { check(false, "the worktree is made"); exit(1) }
        check(w.path == ChatWorktree.path(repo: repo, chat: id, root: root) && w.branch == "coder/" + id && w.source == repo, "the worktree has its path, branch and main checkout")
        check(ChatWorktree.git(["rev-parse", "--abbrev-ref", "HEAD"], in: w.path).output == "coder/" + id, "the worktree is on coder/CHAT")
        check(ChatWorktree.mainCheckout(of: w.path) == nil, "a worktree is not a main checkout (its .git is a file)")
        check((try? String(contentsOfFile: w.path + "/tools/bin/coax", encoding: .utf8)) == "compiler v1", "the listed folder is cloned in")
        check((try? fm.destinationOfSymbolicLink(atPath: w.path + "/tools/bin/back")) == nil, "a link back into the main checkout is cut")
        check((try? fm.destinationOfSymbolicLink(atPath: w.path + "/tools/bin/rel")) == "coax", "a link inside the clone stays")
        try? "compiler v2".write(toFile: w.path + "/tools/bin/coax", atomically: true, encoding: .utf8)
        check((try? String(contentsOfFile: repo + "/tools/bin/coax", encoding: .utf8)) == "compiler v1", "a rebuild in the worktree leaves the main checkout's copy alone")
        check(ChatWorktree.changes(w) == [], "the cloned, ignored tools are not changes")

        // one Cargo target folder per repository, shared by its main checkout and its chats' worktrees (#13771)
        check(ChatWorktree.repository(of: repo) == repo && ChatWorktree.repository(of: repo + "/sub") == repo, "a folder in a main checkout is in that repository")
        check(ChatWorktree.repository(of: w.path) == repo && ChatWorktree.repository(of: w.path + "/tools/bin") == repo, "a chat's worktree is in the repository it was made from")
        check(ChatWorktree.repository(of: tmp + "/home") == nil && ChatWorktree.repository(of: "relative") == nil, "a folder outside a repository is in none")
        let home = ProcessInfo.processInfo.environment["HOME"] ?? ""
        let cargo = ChatWorktree.caches(for: w.path)["CARGO_TARGET_DIR"]
        check(cargo == home + "/.cache/coder/cargo-target/" + ChatWorktree.slug(repo) && cargo?.hasSuffix("/" + (w.path as NSString).deletingLastPathComponent.split(separator: "/").last.map(String.init)!) == true, "a worktree's Cargo target folder is its repository's: \(cargo ?? "none")")
        check(ChatWorktree.caches(for: repo + "/sub")["CARGO_TARGET_DIR"] == cargo, "the main checkout builds into the same folder")
        try? fm.createSymbolicLink(atPath: tmp + "/link", withDestinationPath: tmp + "/work")
        check(ChatWorktree.caches(for: tmp + "/link/my repo/sub")["CARGO_TARGET_DIR"] == cargo, "the repository reached through a link builds into the same folder")
        check(ChatWorktree.path(repo: tmp + "/link/my repo", chat: id, root: root) == w.path, "and its chat's worktree has the same path")
        let other = tmp + "/work/other"
        _ = sh("mkdir -p other && git init -q -b main other", in: tmp + "/work")
        check(ChatWorktree.caches(for: other)["CARGO_TARGET_DIR"].map { $0 != cargo && $0.hasSuffix("/" + ChatWorktree.slug(other)) } == true, "another repository builds into its own folder")
        check(ChatWorktree.caches(for: tmp + "/home")["CARGO_TARGET_DIR"] == nil && ChatWorktree.caches(for: tmp + "/home")["COAX_TOOL_CACHE"] == home + "/.cache/coax-tools", "outside a repository Cargo is left alone; the Coax tool cache is shared")

        // found again on resume, and remade from its branch when its folder was removed by hand
        guard case .success(let again) = ChatWorktree.create(repo: repo, chat: id, root: root) else { check(false, "the worktree is found again"); exit(1) }
        check(again == w, "a chat that goes on finds its worktree")
        _ = sh("echo two > b.txt && \(g) add b.txt && \(g) commit -qm mine", in: w.path)
        _ = ChatWorktree.git(["worktree", "remove", "--force", w.path], in: repo)
        guard case .success(let back) = ChatWorktree.create(repo: repo, chat: id, root: root) else { check(false, "the worktree is remade"); exit(1) }
        check(back == w && fm.fileExists(atPath: w.path + "/b.txt"), "a removed worktree comes back from its branch with its commits")

        // a kept chat's worktree at the path and branch it was given (an older layout) comes back there, even after rm -rf
        let legacy = ChatWorktree(chat: "old", path: root + "/legacy/old", source: repo, branch: "coder/oldshort")
        check((try? ChatWorktree.make(legacy).get()) == legacy && ChatWorktree.git(["rev-parse", "--abbrev-ref", "HEAD"], in: legacy.path).output == "coder/oldshort", "a stored worktree is made at its own path and branch")
        try? fm.removeItem(atPath: legacy.path)
        check((try? ChatWorktree.make(legacy).get()) == legacy && fm.fileExists(atPath: legacy.path + "/a.txt"), "one deleted by hand comes back at its path")
        check(ChatWorktree.remove(legacy) == .removed, "and goes again when clean")

        // kept with the chat: a new store reads it back, and a later keep without it keeps it
        let store = CoderStore()
        store.keep(id: id, title: "Fix the bug", folder: w.path, text: "{}", worktree: w)
        store.keep(id: "other", title: "Other", folder: repo, text: "{}", worktree: w)   // another chat's worktree is not taken
        store.keep(id: id, title: "Fix the bug", folder: w.path, text: "{}")
        let reread = CoderStore()
        check(reread.worktree(id) == w && reread.folder(id) == w.path, "the chat's worktree survives a restart")
        check(reread.worktree("other") == nil, "a chat is kept only with the worktree made for it")
        let raw = (try? String(contentsOf: fm.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0].appendingPathComponent("Coder/chats/index.json"), encoding: .utf8)) ?? ""
        check(raw.contains("\"worktree_path\"") && raw.contains("\"repo_root\"") && raw.contains("\"worktree_branch\""), "index.json names worktree_path, repo_root and worktree_branch")

        // never removed with changes; removed when clean, its branch kept while it has commits nowhere else
        try? "changed\n".write(toFile: w.path + "/a.txt", atomically: true, encoding: .utf8)
        try? "new\n".write(toFile: w.path + "/new.txt", atomically: true, encoding: .utf8)
        check(ChatWorktree.remove(w) == .kept(changes: 2) && fm.fileExists(atPath: w.path + "/a.txt"), "a worktree with changes is kept")
        _ = sh("git checkout -q -- a.txt && rm new.txt", in: w.path)
        check(ChatWorktree.remove(w) == .removed && !fm.fileExists(atPath: w.path), "a clean worktree is removed, ignored tools and all")
        check(ChatWorktree.git(["rev-parse", "--verify", "--quiet", "refs/heads/coder/" + id], in: repo).status == 0, "its branch stays while it has commits nowhere else")
        check(ChatWorktree.remove(w) == .gone, "a worktree already gone is pruned")
        guard case .success(let fresh) = ChatWorktree.create(repo: repo, chat: "aaaa", root: root) else { check(false, "a second worktree"); exit(1) }
        check(ChatWorktree.remove(fresh) == .removed && ChatWorktree.git(["rev-parse", "--verify", "--quiet", "refs/heads/coder/aaaa"], in: repo).status != 0, "a branch with nothing of its own goes with its worktree")
        check(ChatWorktree.create(repo: repo + "/sub", chat: "x", root: root) == .failure(.notARepository), "a folder that is not a main checkout's top gets no worktree")

        print(failed == 0 ? "worktree checks: ok" : "worktree checks: \(failed) FAILED")
        exit(failed == 0 ? 0 : 1)
    }
}
