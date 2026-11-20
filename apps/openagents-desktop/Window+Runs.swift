// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
// Runs: workers wired to pages, the daemon's runs reattached, replies out of view, and syncing with the daemon.
import AppKit

extension CoderWindow {
    /// Marks a background chat unread when it produces output or completes a turn while unfocused.
    private func markUnread(_ r: CoderRun) {
        guard r !== run, let id = r.chat, id != chat else { return }
        if unread.insert(id).inserted { cd_chats(page, chatLines) }
    }
    /// A run's worker lines go to its page, wherever that page is now.
    func wire(_ r: CoderRun) {
        r.worker.line = { [weak self, weak r] text in guard let self, let r else { return }; cd_line(r.page, text); self.followUpBegan(r); self.markUnread(r); self.pump(r) }
        r.worker.gone = { [weak self, weak r] in guard let self, let r else { return }; cd_failed(r.page, "Microcoder stopped running. Send the message again."); if r === self.run { self.recallDue = true }; self.markUnread(r); self.pump(r) }   // a new worker is told the chat so far
    }
    /// After any worker's line: answer what is owed, start the sub-agents the page asks for, and
    /// stop the workers of those that have ended. Microcoder hands tasks over in one line; each runs
    /// on a worker of its own, all at once, and the run's page reads every line. (Each count is
    /// taken before its write: see act.)
    /// Cancel a single running sub-agent without stopping the parent turn or sibling sub-agents (#13766).
    func cancelAgent(_ r: CoderRun, _ j: Int) {
        guard cd_agent_cancel(r.page, Int32(j)) != 0 else { return }
        // kept until the message ends, as pump keeps any ended sub-agent's run (a reopened Coder and the phone read it)
        if let w = r.agents.removeValue(forKey: Int32(j)) { w.gone = nil; w.line = nil; w.stop(forget: false) }
        pump(r)
        wake()
    }
    func stopRun(_ r: CoderRun) {
        var out = [CChar](repeating: 0, count: 4096)
        let n = cd_stop_line(&out, Int32(out.count))
        r.worker.write(out, count: Int(n))
        for (_, one) in r.agents { one.write(out, count: Int(n)) }
    }
    private func pump(_ r: CoderRun) {
        var out = [CChar](repeating: 0, count: 1_048_576)   // a results line: as much as Microcoder's worker reads (MW_LINE)
        let page = r.page, cwd = (r === run ? folder : r.folder).path
        if r !== run && cd_action(page) == 3 { stopRun(r) }
        if cd_budget_exceeded(page) == 0 {
            while true {
                let j = cd_agent_next(page)
                if j < 0 { break }
                let one = CoderWorker()
                if cd_agent_claude(page, j) != 0 { one.program = "claudeworker" }
                one.folder = cwd
                wireAgent(r, j, one)
                guard one.start() else { cd_agent_failed(page, j, one.failure ?? "The sub-agent's worker could not start."); continue }
                r.agents[j] = one
                if let id = one.runID, r.turn != nil { r.turn?.agents.append(id); r.turn?.save() }
                let n = cd_agent_task(page, j, r.session + "-agent-\(j)-" + UUID().uuidString.lowercased(), cwd, &out, Int32(out.count))
                one.write(out, count: Int(n))
            }
        }
        for (j, one) in r.agents {
            let n = cd_agent_answer_line(page, j, &out, Int32(out.count))
            one.write(out, count: Int(n))
            // an ended sub-agent's run is kept until the message ends: a reopened Coder reads it again
            if cd_agent_running(page, j) == 0 { r.agents[j] = nil; one.gone = nil; one.line = nil; one.stop(forget: false) }
        }
        let n = cd_answer_line(page, &out, Int32(out.count)); r.worker.write(out, count: Int(n))
        wake()
    }
    /// Sub-agent j's worker lines go to its run's page.
    private func wireAgent(_ r: CoderRun, _ j: Int32, _ one: CoderWorker) {
        one.line = { [weak self, weak r] text in guard let self, let r, r.agents[j] === one else { return }; cd_agent_line(r.page, j, text); self.markUnread(r); self.pump(r) }
        one.gone = { [weak self, weak r] in guard let self, let r, r.agents[j] === one else { return }; r.agents[j] = nil; cd_agent_failed(r.page, j, "The sub-agent's worker stopped running."); self.markUnread(r); self.pump(r) }
    }
    /// Starts `r`'s worker without blocking the main thread on daemon RPCs, then writes the pending
    /// recall/task lines and records the turn. Works whether `r` stays shown or is detached while starting.
    func sendTask(_ r: CoderRun) {
        guard !r.startingWorker else { return }
        let shown = r === run
        let needsRecall: Bool
        if shown {
            configureWorkerEnv(r, isForeman: isForemanShown)
            syncSkillsNote(for: r.page, isForeman: isForemanShown)
            needsRecall = recallDue
            if needsRecall { recallDue = false }
        } else {
            r.worker.folder = r.folder.path
            needsRecall = false
        }
        let wantProgram = cd_model_agy(r.page) != 0 ? "agyworker" : "mcworker"
        if r.worker.program != wantProgram {
            if r.worker.alive { r.worker.stop(forget: false) }
            r.worker.program = wantProgram
        }
        r.startingWorker = true
        r.worker.startAsync { [weak self, weak r] ok in
            guard let self, let r else { return }
            r.startingWorker = false
            let stillShown = r === self.run
            let stillBackground = !stillShown && r.chat != nil && self.runs[r.chat!] === r
            guard stillShown || stillBackground else {
                if ok { r.worker.stop() }
                return
            }
            guard cd_running(r.page) != 0 else { return }
            guard ok else {
                cd_failed(r.page, r.worker.failure ?? "Microcoder could not start.")
                self.wake()
                return
            }
            var out = [CChar](repeating: 0, count: 262144)
            let cwd = (stillShown ? self.folder : r.folder).path
            let from = r.worker.offset
            if needsRecall {
                // an opened chat's turns, or nothing to remember when it has none that ended
                var n = cd_recall_line(r.page, &out, Int32(out.count))
                if n == 0 { n = cd_forget_line(&out, Int32(out.count)) }
                r.worker.write(out, count: Int(n))
            }
            let n = cd_task(r.page, r.session, cwd, &out, Int32(out.count))
            r.worker.write(out, count: Int(n))
            self.beginTurn(r, from: from)
            self.bindQueue(r)
            self.wake()
        }
    }
    /// A message sent on a daemon run is kept (CoderTurn) with the chat as it is now, so a Coder
    /// that quits and opens again attaches to it. `from` is where its lines start in the run's output.
    func beginTurn(_ r: CoderRun, from: UInt64) {
        guard let runID = r.worker.runID else { return }
        let shown = r === run
        keep(shown ? nil : r)
        guard let id = shown ? chat : r.chat, let text = store.text(id) else { return }
        let t = CoderTurn(run: runID, chat: id, offset: from, folder: (shown ? folder : r.folder).path,
                          session: r.session, program: r.worker.program, agents: [])
        r.turn = t
        t.save(chatText: text)
    }
    /// At launch: every message whose run the daemon kept while Coder was closed comes back as a
    /// chat whose reply runs out of view. Its page sends the message again without a worker and
    /// reads what the run wrote since (replay); then it follows the run live, or ends as completed.
    /// A run no kept message names (an idle worker of a Coder that crashed) is stopped, and its files go.
    func reattach() {
        guard let daemon = CoderWorker.daemon, daemon.ownsRuns else { return }
        var named = Set<String>()
        for t in CoderTurn.all() {
            guard CoderWorker.runState(t.run) != nil, store.meta(t.chat) != nil, runs[t.chat] == nil,
                  let kept = t.chatText, let sent = CoderTurn.withoutLast(kept), let back = cd_headless() else {
                CoderTurn.remove(t.run); for id in t.agents { daemon.forgetRun(id) }; continue
            }
            coderApplyOpenRouter(back); cd_claude(back, CoderWorker.hasClaude ? 1 : 0); cd_agy(back, CoderWorker.hasAntigravity ? 1 : 0)
            syncSkillsNote(for: back, isForeman: t.chat == Foreman.chatID)
            var out = [CChar](repeating: 0, count: 1_048_576)   // a results line: as much as Microcoder's worker reads (MW_LINE)
            guard cd_load(back, sent.chat) != 0, cd_set_text(back, sent.ask) != 0, cd_submit(back) != 0, cd_action(back) == 1 else {
                cd_close(back); CoderTurn.remove(t.run); continue
            }
            _ = cd_task(back, t.session, t.folder, &out, Int32(out.count))   // the worker had it when the message was sent
            let r = CoderRun(page: back, folder: URL(fileURLWithPath: t.folder))
            r.chat = t.chat; r.session = t.session; r.turn = t; r.worker.program = t.program
            r.worker.attach(t.run, from: t.offset)
            replay(r)
            followUpBegan(r)   // the worker started a queued message while Coder was closed
            wire(r); r.worker.follow()
            runs[t.chat] = r
            if t.chat != chat { unread.insert(t.chat) }
            named.insert(t.run); named.formUnion(r.turn?.agents ?? [])
            pump(r)
        }
        if let live = daemon.listRuns() {
            for one in live where !named.contains(one.run_id) {
                let id = one.run_id
                if one.isLive { daemon.stopRun(id) { DispatchQueue.main.asyncAfter(deadline: .now() + 4) { daemon.forgetRun(id) } } } else { daemon.forgetRun(id) }
            }
        }
        cd_chats(page, chatLines); syncForemanUI()
    }
    /// The run's lines since the message was sent go to its page as they came. A line after one
    /// that owed the worker an answer means the answer was given, so it is not given again.
    private func replay(_ r: CoderRun) {
        var out = [CChar](repeating: 0, count: 1_048_576)   // a results line: as much as Microcoder's worker reads (MW_LINE)
        var started = r.turn?.agents ?? []
        for (k, text) in r.worker.readAvailable().enumerated() {
            if k > 0 { _ = cd_answer_line(r.page, &out, Int32(out.count)) }
            cd_line(r.page, text)
            resumeAgents(r, &started, &out)
        }
    }
    /// The sub-agents a replayed line handed over: each attaches to the run started for it, in
    /// order, and its lines are read the same way; one with no run yet starts now.
    private func resumeAgents(_ r: CoderRun, _ started: inout [String], _ out: inout [CChar]) {
        let page = r.page
        while true {
            let j = cd_agent_next(page)
            if j < 0 { break }
            let one = CoderWorker()
            if cd_agent_claude(page, j) != 0 { one.program = "claudeworker" }
            one.folder = r.folder.path
            let n = cd_agent_task(page, j, r.session + "-agent-\(j)-" + UUID().uuidString.lowercased(), r.folder.path, &out, Int32(out.count))
            if !started.isEmpty {
                one.attach(started.removeFirst(), from: 0)
                var owed = [CChar](repeating: 0, count: 4096)
                for (m, text) in one.readAvailable().enumerated() {
                    if m > 0 { _ = cd_agent_answer_line(page, j, &owed, Int32(owed.count)) }
                    cd_agent_line(page, j, text)
                }
            } else {
                guard one.start() else { cd_agent_failed(page, j, one.failure ?? "The sub-agent's worker could not start."); continue }
                one.write(out, count: Int(n))
                if let id = one.runID { r.turn?.agents.append(id); r.turn?.save() }
            }
            wireAgent(r, j, one)
            r.agents[j] = one
            if cd_agent_running(page, j) == 0 { r.agents[j] = nil; one.gone = nil; one.line = nil; one.stop(forget: false) }
            else { one.follow() }
        }
    }
    /// The list as the page reads it: one chat a line, its id, a tab, its title, and optional tabs
    /// for busy, pinned, managed and unread flags.
    var chatLines: String {
        store.orderedList.map { chat in
            let isBusy = runs[chat.id] != nil
            let isPinned = chat.pinned != nil
            let isManaged = chat.managedBy == Foreman.managedTag
            let isUnread = unread.contains(chat.id)
            return chat.id + "\t" + chat.title + "\t" + (isBusy ? "1" : "0") + "\t" + (isPinned ? "1" : "0") + "\t" + (isManaged ? "1" : "0") + "\t" + (isUnread ? "1" : "0")
        }.joined(separator: "\n")
    }
    var isForemanShown: Bool { chat == Foreman.chatID }
    func syncFromDaemon() {
        daemonClient.readForeman { [weak self] state in
            guard let self, let rawTier = state?.tier, let tier = ForemanTier(rawValue: rawTier) else { return }
            if self.foremanTier != tier {
                self.foremanTier = tier
                tier.save()
                self.syncForemanUI()
                self.wake()
            }
        }
        daemonClient.listSessionMeta { [weak self] items in
            guard let self, !items.isEmpty else { return }
            var changed = false
            for item in items {
                guard self.store.meta(item.session_id) != nil else { continue }
                if let title = item.title, !title.isEmpty, self.store.title(item.session_id) != title {
                    self.store.rename(item.session_id, to: title)
                    changed = true
                }
                if let pinned = item.pinned, self.store.isPinned(item.session_id) != pinned {
                    self.store.togglePin(item.session_id)
                    changed = true
                }
            }
            if changed {
                cd_chats(self.page, self.chatLines)
                self.wake()
            }
        }
    }
    func syncForemanUI() {
        let busy = runs[Foreman.chatID] != nil || (chat == Foreman.chatID && cd_running(page) != 0)
        cd_foreman(page, busy ? 1 : 0, Int32(foremanTier.rawValue))
    }
    func syncSkillsNote(for pagePtr: UnsafeMutableRawPointer, isForeman: Bool) {
        let base = coderSkillsNote()
        if isForeman {
            let note = Foreman.systemPrompt(tier: foremanTier, workspace: folder.path, chatCount: store.orderedList.count, runningCount: runningCount, baseSkills: base)
            cd_skills(pagePtr, note)
        } else {
            cd_skills(pagePtr, base)
        }
    }
    func configureWorkerEnv(_ r: CoderRun, isForeman: Bool) {
        var env: [String: String] = [:]
        if isForeman && foremanServer.port > 0 {
            env = [
                "CODER_FOREMAN_URL": foremanServer.baseURL,
                "CODER_FOREMAN_TOKEN": foremanServer.token,
                "CODER_FOREMAN_TIER": foremanTier.wireName
            ]
            if let res = Bundle.main.resourcePath {
                env["PATH"] = res
            }
        }
        let cwd = (r === run ? folder : r.folder).path
        let changed = r.worker.extraEnv != env || ChatWorktree.caches(for: r.worker.folder) != ChatWorktree.caches(for: cwd)
        r.worker.extraEnv = env
        r.worker.folder = cwd
        if changed && r.worker.alive && cd_running(r.page) == 0 {
            r.worker.stop()
            if r === run { recallDue = true }
        }
    }
    func stopAllChats() {
        if cd_running(page) != 0 { stopRun(run) }
        let ids = Array(runs.keys)
        for id in ids {
            if let back = runs.removeValue(forKey: id) {
                stopRun(back)
                keep(back)
                back.stop()
                cd_close(back.page)
            }
        }
        cd_chats(page, chatLines)
        syncForemanUI()
        wake()
    }
    /// The shown chat's reply goes on out of view: its run moves to a page with no window and the
    /// window gets a new run. True when nothing runs in the window now.
    @discardableResult func detach() -> Bool {
        if cd_running(page) == 0 {
            if !run.preparingWorktree, cd_save_due(page) != 0 { keep() }
            return true
        }
        keep()
        guard let id = chat, let back = cd_headless() else { return false }
        coderApplyOpenRouter(back)
        guard cd_take(back, page) != 0 else { cd_close(back); return false }
        let moved = run
        moved.page = back; moved.folder = folder; moved.chat = id
        runs[id] = moved
        run = CoderRun(page: page, folder: folder); wire(run)
        chat = nil; recallDue = false; cd_current(page, "")
        configureWorkerEnv(run, isForeman: false)
        syncSkillsNote(for: page, isForeman: false)
        cd_chats(page, chatLines); syncForemanUI()
        wake()
        return true
    }
    /// A reply out of view ended: its chat is kept, its workers stop and its page goes.
    func finish(_ id: String) {
        guard let back = runs.removeValue(forKey: id) else { return }
        let cost = Int(cd_cost_usd_micros(back.page))
        back.stop(); cd_close(back.page)
        _ = foremanEvents.emit(type: "chat.finished", chatID: id, data: [
            "title": store.title(id) ?? "Chat",
            "cost_usd_micros": cost
        ])
        if id != chat { unread.insert(id) }
        cd_chats(page, chatLines); syncForemanUI(); wake()
    }
}
