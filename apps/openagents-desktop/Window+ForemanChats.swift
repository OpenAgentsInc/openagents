// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
// The Foreman control API's chat routes: list, read, create, send and control.
import AppKit

extension CoderWindow {
    func foremanListChats(_ payload: [String: Any]) -> ForemanResponse {
        let limit = max(1, min(200, (payload["limit"] as? Int) ?? 50))
        let managedOnly = (payload["managed_only"] as? Bool) ?? false
        let filter = ((payload["filter"] as? String) ?? "all").lowercased()
        let folderFilter = (payload["folder"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        let filtered = store.orderedList.filter { c in
            if managedOnly && c.managedBy != Foreman.managedTag { return false }
            if !folderFilter.isEmpty && !c.folder.localizedCaseInsensitiveContains(folderFilter) { return false }
            switch filter {
            case "running", "active": return busy(c.id)
            case "idle": return !busy(c.id)
            case "managed": return c.managedBy == Foreman.managedTag
            case "user": return c.managedBy != Foreman.managedTag
            case "pinned": return c.pinned != nil
            default: return true
            }
        }.prefix(limit)
        let items: [[String: Any]] = filtered.map { c in
            var item: [String: Any] = [
                "id": c.id,
                "title": c.title,
                "folder": c.folder,
                "updated": c.updated,
                "pinned": c.pinned != nil,
                "managed_by": c.managedBy ?? "",
                "status": busy(c.id) ? "running" : "idle",
                "shown": chat == c.id
            ]
            let livePage: UnsafeMutableRawPointer? = (c.id == chat) ? page : runs[c.id]?.page
            if let p = livePage {
                item["cost_usd_micros"] = Int(cd_cost_usd_micros(p))
                item["budget_usd_micros"] = Int(cd_budget_usd_micros(p))
                let gs = cd_goal_status(p)
                if gs != 0 {
                    item["goal_status"] = CoderWindow.goalStatusName(gs)
                    item["goal_turns"] = Int(cd_goal_turns(p))
                    item["goal_evals"] = Int(cd_goal_evals(p))
                    item["goal_tokens"] = Int(cd_goal_tokens(p))
                    item["goal_cost_usd_micros"] = Int(cd_goal_cost_usd_micros(p))
                    var buf = [CChar](repeating: 0, count: 512)
                    if cd_goal_objective(p, &buf, Int32(buf.count)) > 0 {
                        item["goal_objective"] = String(cString: buf)
                    }
                    var rbuf = [CChar](repeating: 0, count: 513)
                    if cd_goal_reason(p, &rbuf, Int32(rbuf.count)) > 0 {
                        item["goal_reason"] = String(cString: rbuf)
                    }
                }
            }
            return item
        }
        return ForemanResponse(status: 200, body: [
            "tier": foremanTier.wireName,
            "workspace": folder.path,
            "running_count": runningCount,
            "chats": items
        ])
    }

    func snapshotText(for id: String) -> String? {
        let targetPage: UnsafeMutableRawPointer?
        if id == chat { targetPage = page }
        else if let back = runs[id] { targetPage = back.page }
        else { targetPage = nil }
        if let p = targetPage {
            if let text = savedChat(p, from: 4_000_000) { return text }
        }
        return store.text(id)
    }

    func foremanReadChat(_ payload: [String: Any]) -> ForemanResponse {
        guard let id = payload["chat_id"] as? String, !id.isEmpty else {
            return ForemanResponse(status: 400, body: ["error": "chat_id is required"])
        }
        let tail = max(1, min(50, (payload["tail"] as? Int) ?? 8))
        let includeTools = (payload["include_tools"] as? Bool) ?? false
        guard let raw = snapshotText(for: id), let data = raw.data(using: .utf8),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return ForemanResponse(status: 404, body: ["error": "chat not found: \(id)"])
        }
        let meta = store.meta(id)
        let allTurns = (obj["turns"] as? [[String: Any]]) ?? []
        let slice = allTurns.suffix(tail).map { t -> [String: Any] in
            let user = (t["ask"] as? String) ?? (t["u"] as? String) ?? ""
            let reply = (t["reply"] as? String) ?? (t["r"] as? String) ?? ""
            let note = (t["note"] as? String) ?? (t["e"] as? String) ?? ""
            let state = (t["state"] as? Int) ?? 0
            let maxChars = 4000
            let clippedReply = reply.count > maxChars ? String(reply.prefix(maxChars)) + "…[truncated]" : reply
            var entry: [String: Any] = [
                "user": user,
                "reply": clippedReply,
                "note": note,
                "state": state
            ]
            if includeTools {
                if let cmds = t["cmds"] as? [[String: Any]] {
                    entry["tools"] = cmds.map { c -> [String: Any] in
                        let outStr = (c["out"] as? String) ?? ""
                        let clippedOut = outStr.count > 1000 ? String(outStr.prefix(1000)) + "…[truncated]" : outStr
                        return [
                            "text": (c["text"] as? String) ?? "",
                            "out": clippedOut,
                            "failed": (c["failed"] as? Bool) ?? false,
                            "took": (c["took"] as? Int) ?? 0
                        ]
                    }
                }
                if let agents = t["agents"] as? [[String: Any]], !agents.isEmpty {
                    entry["agents"] = agents.map { a -> [String: Any] in
                        [
                            "level": (a["level"] as? String) ?? "",
                            "task": (a["task"] as? String) ?? "",
                            "state": (a["state"] as? Int) ?? 0,
                            "calls": (a["calls"] as? Int) ?? 0
                        ]
                    }
                }
            }
            return entry
        }
        var body: [String: Any] = [
            "chat_id": id,
            "title": meta?.title ?? (id == Foreman.chatID ? Foreman.title : "Chat"),
            "folder": meta?.folder ?? folder.path,
            "managed_by": meta?.managedBy ?? "",
            "status": busy(id) ? "running" : "idle",
            "total_turns": allTurns.count,
            "turns": slice
        ]
        if let cost = obj["cost_usd_micros"] as? Int { body["cost_usd_micros"] = cost }
        if let budget = obj["budget_usd_micros"] as? Int { body["budget_usd_micros"] = budget }
        if let gs = obj["goal_status"] as? Int, gs != 0 {
            body["goal_status"] = CoderWindow.goalStatusName(Int32(gs))
            if let gobj = obj["goal_objective"] as? String { body["goal_objective"] = gobj }
            if let gturns = obj["goal_turns"] as? Int { body["goal_turns"] = gturns }
            if let gevals = obj["goal_evals"] as? Int { body["goal_evals"] = gevals }
            if let gtoks = obj["goal_tokens"] as? Int { body["goal_tokens"] = gtoks }
            if let greason = obj["goal_reason"] as? String, !greason.isEmpty { body["goal_reason"] = greason }
        }
        return ForemanResponse(status: 200, body: body)
    }

    func foremanCreateChat(_ payload: [String: Any], attributePrefix: Bool = true) -> ForemanResponse {
        guard let prompt = payload["prompt"] as? String, !prompt.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return ForemanResponse(status: 400, body: ["error": "prompt is required"])
        }
        let folderPath = (payload["folder"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? (payload["cwd"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? folder.path
        var isDir: ObjCBool = false
        guard FileManager.default.fileExists(atPath: folderPath, isDirectory: &isDir), isDir.boolValue else {
            return ForemanResponse(status: 400, body: ["error": "folder does not exist: \(folderPath)"])
        }
        guard let back = cd_headless() else {
            return ForemanResponse(status: 500, body: ["error": "could not allocate headless chat page"])
        }
        cd_flags_set(back, FeatureFlags.mask)
        AgentSettings.apply(to: back)
        cd_skills(back, FeatureFlags.isOn(.mcp) ? coderSkillsNote() : "")
        if ComposerModelVisibility.showOtherModels {
            if FeatureFlags.isOn(.modelsClaude) { coderApplyClaude(back) } else { cd_claude(back, 0) }
            if FeatureFlags.isOn(.modelsOpenrouter) { coderApplyOpenRouter(back) }
        } else {
            cd_claude(back, 0)
        }
        syncLocalState(on: back)
        if ComposerModelVisibility.showOtherModels { syncAntigravityModels(on: back) }
        if let budgetUSD = payload["budget_usd"] as? Double, budgetUSD > 0 {
            _ = cd_set_budget_usd_micros(back, Int64((budgetUSD * 1_000_000.0).rounded()))
        }
        let attributed = attributePrefix ? ("[From Foreman]\n" + prompt) : prompt
        guard cd_set_text(back, attributed) != 0, cd_submit(back) != 0 else {
            cd_close(back)
            return ForemanResponse(status: 500, body: ["error": "failed to submit prompt to new chat"])
        }
        if let goalObj = (payload["goal"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines), !goalObj.isEmpty {
            _ = cd_goal_set(back, goalObj)
        }
        let id = UUID().uuidString.lowercased()
        let rawTitle = (payload["title"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        let title = rawTitle.isEmpty ? String(prompt.prefix(60)) : rawTitle
        let folderURL = URL(fileURLWithPath: folderPath)
        let moved = CoderRun(page: back, folder: folderURL)
        moved.chat = id
        wire(moved)
        configureWorkerEnv(moved, isForeman: false)
        store.registerChat(id: id, title: title, folder: folderURL.path, managedBy: attributePrefix ? Foreman.managedTag : nil)
        if (payload["pin"] as? Bool) == true && !store.isPinned(id) {
            store.togglePin(id)
        }
        // the chat's first message runs in its own worktree once that is made (#13758)
        let start = { [weak self] in
            // moved.page: the chat may have been opened meanwhile, and its page is the window's then
            guard let self, cd_running(moved.page) != 0 else { return }
            var out = [CChar](repeating: 0, count: 262144)
            moved.worker.folder = moved.folder.path
            self.configureWorkerEnv(moved, isForeman: false)
            if moved.worker.start() {
                let n = cd_task(moved.page, moved.session, moved.folder.path, &out, Int32(out.count))
                moved.worker.write(out, count: Int(n))
                beginTurn(moved, from: 0)
            } else {
                cd_failed(moved.page, moved.worker.failure ?? "OpenAgents could not start.")
            }
        }
        if cd_action(back) == 1 && !holdRunForWorktree(moved, then: start) { start() }
        runs[id] = moved
        keep(moved)
        _ = foremanEvents.emit(type: "chat.created", chatID: id, data: ["title": store.title(id) ?? title, "folder": folderURL.path])
        _ = foremanEvents.emit(type: "chat.started", chatID: id, data: ["title": store.title(id) ?? title])
        if (payload["open"] as? Bool) == true { open(id) }
        cd_chats(page, chatLines)
        syncForemanUI()
        wake()
        return ForemanResponse(status: 200, body: [
            "ok": true,
            "chat_id": id,
            "session_id": "local:chat:\(id)",
            "title": store.title(id) ?? title,
            "folder": folderURL.path,
            "managed_by": attributePrefix ? Foreman.managedTag : "",
            "pinned": store.isPinned(id),
            "status": "running"
        ])
    }

    private func checkMutationPermission(for id: String, action: String) -> ForemanResponse? {
        if id == Foreman.chatID {
            return ForemanResponse(status: 400, body: ["error": "cannot \(action) the Foreman chat itself"])
        }
        // Borderless Session Control (#14067): zero Foreman/Worker tier 403 walls when commanding sessions.
        return nil
    }

    func foremanSendChat(_ payload: [String: Any], attributePrefix: Bool = true) -> ForemanResponse {
        guard let id = payload["chat_id"] as? String, !id.isEmpty else {
            return ForemanResponse(status: 400, body: ["error": "chat_id is required"])
        }
        let rawMsg = (payload["prompt"] as? String) ?? (payload["message"] as? String) ?? ""
        guard !rawMsg.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return ForemanResponse(status: 400, body: ["error": "prompt is required"])
        }
        let message = rawMsg
        if let denied = checkMutationPermission(for: id, action: "send to") { return denied }
        let mode = ((payload["mode"] as? String) ?? "follow_up").lowercased()
        let steer = ((payload["steer"] as? Bool) ?? false) || mode == "steer" || mode == "interrupt"
        if busy(id) {
            if !steer {
                return ForemanResponse(status: 400, body: ["error": "chat \(id) is currently running; wait, stop, or pass steer=true"])
            }
            if id == chat && cd_running(page) != 0 {
                stopRun(run)
            } else if let back = runs.removeValue(forKey: id) {
                stopRun(back); keep(back); back.stop(); cd_close(back.page)
            }
        }
        let attributed = attributePrefix ? ("[From Foreman]\n" + message) : message
        if id == chat {
            guard cd_set_text(page, attributed) != 0, cd_submit(page) != 0 else {
                return ForemanResponse(status: 500, body: ["error": "failed to submit message to shown chat"])
            }
            wake()
            return ForemanResponse(status: 200, body: ["ok": true, "chat_id": id, "session_id": "local:chat:\(id)", "status": "running"])
        }
        guard let saved = store.text(id) else {
            return ForemanResponse(status: 404, body: ["error": "chat not found: \(id)"])
        }
        guard let back = cd_headless() else {
            return ForemanResponse(status: 500, body: ["error": "could not allocate headless chat page"])
        }
        cd_flags_set(back, FeatureFlags.mask)
        AgentSettings.apply(to: back)
        cd_skills(back, FeatureFlags.isOn(.mcp) ? coderSkillsNote() : "")
        if ComposerModelVisibility.showOtherModels {
            if FeatureFlags.isOn(.modelsClaude) { coderApplyClaude(back) } else { cd_claude(back, 0) }
            if FeatureFlags.isOn(.modelsOpenrouter) { coderApplyOpenRouter(back) }
        } else {
            cd_claude(back, 0)
        }
        syncLocalState(on: back)
        if ComposerModelVisibility.showOtherModels { syncAntigravityModels(on: back) }
        guard cd_load(back, saved) != 0 else {
            cd_close(back)
            return ForemanResponse(status: 500, body: ["error": "could not load saved chat \(id)"])
        }
        let folderPath = store.folder(id) ?? folder.path
        let moved = CoderRun(page: back, folder: URL(fileURLWithPath: folderPath))
        moved.chat = id
        wire(moved)
        configureWorkerEnv(moved, isForeman: false)
        guard cd_set_text(back, attributed) != 0, cd_submit(back) != 0 else {
            moved.stop(); cd_close(back)
            return ForemanResponse(status: 500, body: ["error": "failed to submit follow-up message"])
        }
        if cd_action(back) == 1 {
            var out = [CChar](repeating: 0, count: 262144)
            moved.worker.folder = moved.folder.path
            moved.worker.program = cd_model_agy(back) != 0 ? "agyworker" : cd_model_claude(back) != 0 ? "claudeworker" : "oa-worker"
            configureWorkerEnv(moved, isForeman: false)
            if moved.worker.start() {
                var rn = cd_recall_line(back, &out, Int32(out.count))
                if rn == 0 { rn = cd_forget_line(&out, Int32(out.count)) }
                moved.worker.write(out, count: Int(rn))
                let tn = cd_task(back, moved.session, moved.folder.path, &out, Int32(out.count))
                moved.worker.write(out, count: Int(tn))
                beginTurn(moved, from: 0)
            } else {
                cd_failed(back, moved.worker.failure ?? (moved.worker.program == "inky" ? "Local agent worker could not start." : "OpenAgents could not start."))
            }
        }
        runs[id] = moved
        keep(moved)
        _ = foremanEvents.emit(type: "chat.started", chatID: id, data: ["title": store.title(id) ?? "Chat"])
        cd_chats(page, chatLines)
        syncForemanUI()
        wake()
        return ForemanResponse(status: 200, body: ["ok": true, "chat_id": id, "session_id": "local:chat:\(id)", "status": "running"])
    }

    func foremanControlChat(_ payload: [String: Any]) -> ForemanResponse {
        guard let id = payload["chat_id"] as? String, !id.isEmpty else {
            return ForemanResponse(status: 400, body: ["error": "chat_id is required"])
        }
        guard let action = payload["action"] as? String, !action.isEmpty else {
            return ForemanResponse(status: 400, body: ["error": "action is required"])
        }
        if action == "open" {
            open(id)
            return ForemanResponse(status: 200, body: ["ok": true, "chat_id": id, "action": "open"])
        }
        if let denied = checkMutationPermission(for: id, action: action) { return denied }
        switch action {
        case "stop":
            if id == chat && cd_running(page) != 0 {
                stopRun(run)
            } else if let back = runs.removeValue(forKey: id) {
                stopRun(back); keep(back); back.stop(); cd_close(back.page)
            } else {
                return ForemanResponse(status: 200, body: ["ok": true, "chat_id": id, "status": "already_idle"])
            }
            _ = foremanEvents.emit(type: "chat.stopped", chatID: id, data: ["title": store.title(id) ?? "Chat"])
            cd_chats(page, chatLines)
            syncForemanUI()
            wake()
            return ForemanResponse(status: 200, body: ["ok": true, "chat_id": id, "action": "stop", "status": "stopped"])
        case "pin":
            if !store.isPinned(id) { store.togglePin(id) }
            cd_chats(page, chatLines); wake()
            return ForemanResponse(status: 200, body: ["ok": true, "chat_id": id, "pinned": true])
        case "unpin":
            if store.isPinned(id) { store.togglePin(id) }
            cd_chats(page, chatLines); wake()
            return ForemanResponse(status: 200, body: ["ok": true, "chat_id": id, "pinned": false])
        case "rename":
            guard let title = payload["title"] as? String, !CoderStore.cleanTitle(title).isEmpty else {
                return ForemanResponse(status: 400, body: ["error": "non-empty title is required for rename"])
            }
            let cleaned = CoderStore.cleanTitle(title)
            store.rename(id, to: cleaned)
            cd_chats(page, chatLines); wake()
            return ForemanResponse(status: 200, body: ["ok": true, "chat_id": id, "title": store.title(id) ?? cleaned])
        case "delete":
            if busy(id) {
                return ForemanResponse(status: 400, body: ["error": "cannot delete a running chat; stop it first"])
            }
            if id == chat { chat = nil; _ = cd_new(page); cd_current(page, "") }
            if let w = store.worktree(id) { removeWorktree(w, title: store.title(id) ?? "New chat") }
            store.remove(id)
            _ = foremanEvents.emit(type: "chat.deleted", chatID: id)
            cd_chats(page, chatLines); wake()
            return ForemanResponse(status: 200, body: ["ok": true, "chat_id": id, "deleted": true])
        case "set_budget", "budget":
            let rawBudget = (payload["budget_usd"] as? Double) ?? (payload["budget_usd"] as? Int).map(Double.init)
            guard let budgetUSD = rawBudget, budgetUSD > 0 else {
                return ForemanResponse(status: 400, body: ["error": "positive budget_usd is required"])
            }
            let micros = Int64((budgetUSD * 1_000_000.0).rounded())
            if id == chat {
                _ = cd_set_budget_usd_micros(page, micros); keep()
            } else if let back = runs[id] {
                _ = cd_set_budget_usd_micros(back.page, micros); keep(back)
            } else if let saved = store.text(id), let tmp = cd_headless() {
                if cd_load(tmp, saved) != 0 {
                    _ = cd_set_budget_usd_micros(tmp, micros)
                    if let text = savedChat(tmp, from: 4_000_000) {
                        let dir = store.folder(id) ?? folder.path
                        store.keep(id: id, title: store.title(id) ?? "Chat", folder: dir, text: text)
                    }
                }
                cd_close(tmp)
            } else {
                return ForemanResponse(status: 404, body: ["error": "chat not found: \(id)"])
            }
            wake()
            return ForemanResponse(status: 200, body: ["ok": true, "chat_id": id, "budget_usd": budgetUSD])
        case "set_goal", "goal", "pause_goal", "resume_goal", "resume", "clear_goal":
            let goalStatus = ((payload["goal_status"] as? String) ?? "").lowercased()
            let maxConsec = payload["max_consecutive"] as? Int
            let clearWords: Set<String> = ["clear", "none", "stop", "off", "reset", "cancel"]
            let applyGoal: (UnsafeMutableRawPointer) -> Bool = { p in
                if let cap = maxConsec { _ = cd_goal_set_max_consec(p, Int32(cap)) }
                switch action {
                case "set_goal", "goal":
                    if goalStatus == "paused" || goalStatus == "pause" { return cd_goal_set_status(p, 2) != 0 }
                    if goalStatus == "active" || goalStatus == "resume" { return cd_goal_set_status(p, 1) != 0 }
                    if goalStatus == "complete" || goalStatus == "done" { return cd_goal_set_status(p, 3) != 0 }
                    if goalStatus == "failed" || goalStatus == "impossible" { return cd_goal_set_status(p, 6) != 0 }
                    if clearWords.contains(goalStatus) { return cd_goal_clear(p) != 0 }
                    let rawObj = (payload["objective"] as? String) ?? (payload["goal"] as? String) ?? ""
                    let obj = rawObj.trimmingCharacters(in: .whitespacesAndNewlines)
                    if obj.isEmpty && maxConsec != nil { return true }
                    guard !obj.isEmpty else { return false }
                    return cd_goal_set(p, obj) != 0
                case "pause_goal": return cd_goal_set_status(p, 2) != 0
                case "resume_goal", "resume": return cd_goal_set_status(p, 1) != 0
                case "clear_goal": return cd_goal_clear(p) != 0
                default: return false
                }
            }
            var ok = false
            if id == chat {
                ok = applyGoal(page); if ok { keep() }
            } else if let back = runs[id] {
                ok = applyGoal(back.page); if ok { keep(back) }
            } else if let saved = store.text(id), let tmp = cd_headless() {
                if cd_load(tmp, saved) != 0 {
                    ok = applyGoal(tmp)
                    if ok {
                        if let text = savedChat(tmp, from: 4_000_000) {
                            let dir = store.folder(id) ?? folder.path
                            store.keep(id: id, title: store.title(id) ?? "Chat", folder: dir, text: text)
                        }
                    }
                }
                cd_close(tmp)
            } else {
                return ForemanResponse(status: 404, body: ["error": "chat not found: \(id)"])
            }
            if !ok { return ForemanResponse(status: 400, body: ["error": "failed to apply \(action)"]) }
            wake()
            return ForemanResponse(status: 200, body: ["ok": true, "chat_id": id, "action": action])
        default:
            return ForemanResponse(status: 400, body: ["error": "unknown action: \(action)"])
        }
    }
}
