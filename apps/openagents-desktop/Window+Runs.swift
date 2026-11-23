// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
// Runs: workers wired to pages, the daemon's runs reattached, replies out of view, and syncing with the daemon.
import AppKit

extension CoderWindow {
    /// Marks a background chat unread when it produces output or completes a turn while unfocused.
    private func markUnread(_ r: CoderRun) {
        guard r !== run, let id = r.chat, id != chat else { return }
        if unread.insert(id).inserted { cd_chats(page, chatLines); updateDockBadge() }
    }
    /// A run's worker lines go to its page, wherever that page is now.
    func wire(_ r: CoderRun) {
        r.worker.line = { [weak self, weak r] text in
            guard let self, let r else { return }
            if r.worker.program == "agyworker" && text.contains("Sign in to Antigravity in Settings") {
                ProviderInstancesStore.shared.refreshAll()
            }
            cd_line(r.page, text)
            if cd_running(r.page) == 0 {
                r.stopRequestedAt = 0
                r.stopGen &+= 1
            }
            self.followUpBegan(r)
            self.markUnread(r)
            self.pump(r)
        }
        r.worker.gone = { [weak self, weak r] in guard let self, let r else { return }; r.stopRequestedAt = 0; r.stopGen &+= 1; let label = r.worker.program == "inky" ? "Local agent worker" : r.worker.program == "claudeworker" ? "Claude" : r.worker.program == "agyworker" ? "Antigravity" : "Microcoder"; cd_failed(r.page, "\(label) stopped running. Send the message again."); if r === self.run { self.recallDue = true }; self.markUnread(r); self.notifyTurnFailed(chatID: r === self.run ? self.chat : r.chat, detail: "\(label) stopped running.", isActiveChat: r === self.run); self.pump(r) }   // a new worker is told the chat so far
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
    /// Immediately terminates an unresponsive/stuck worker and finalizes the running turn as stopped.
    func forceStopRun(_ r: CoderRun) {
        r.startGen &+= 1
        r.startingWorker = false
        r.preparingWorktree = false
        r.stopRequestedAt = 0
        r.stopGen &+= 1
        r.worker.gone = nil
        r.worker.line = nil
        if r.worker.alive { r.worker.stop(forget: false) }
        for (_, one) in r.agents {
            one.gone = nil
            one.line = nil
            one.stop(forget: false)
        }
        r.agents = [:]
        wire(r)
        if r === run { recallDue = true }
        cd_turn_loading_model(r.page, 0)
        if cd_running(r.page) != 0 {
            cd_line(r.page, "{\"kind\":\"ended\",\"ending\":\"stopped\",\"text\":\"\"}")
        }
        r.endTurn()
        if r === run { keep() } else { keep(r) }
        pump(r)
        wake()
    }
    func stopRun(_ r: CoderRun) {
        if r.startingWorker || r.preparingWorktree || !r.worker.alive {
            forceStopRun(r)
            return
        }
        // Second Stop click while already stopping (e.g. hung/idle worker) force-stops immediately.
        if r.stopRequestedAt > 0 {
            forceStopRun(r)
            return
        }
        r.stopRequestedAt = CACurrentMediaTime()
        r.stopGen &+= 1
        let gen = r.stopGen
        var out = [CChar](repeating: 0, count: 4096)
        let n = cd_stop_line(&out, Int32(out.count))
        r.worker.write(out, count: Int(n))
        for (_, one) in r.agents { one.write(out, count: Int(n)) }
        // Escalate automatically if the worker fails to acknowledge stop within 3.5s.
        DispatchQueue.main.asyncAfter(deadline: .now() + 3.5) { [weak self, weak r] in
            guard let self, let r, r.stopGen == gen, r.stopRequestedAt > 0, cd_running(r.page) != 0 else { return }
            let stillActive = (r === self.run) || (r.chat != nil && self.runs[r.chat!] === r)
            guard stillActive else { return }
            self.forceStopRun(r)
        }
    }
    func pump(_ r: CoderRun) {
        var out = [CChar](repeating: 0, count: 1_048_576)   // a results line: as much as Microcoder's worker reads (MW_LINE)
        let page = r.page, cwd = (r === run ? folder : r.folder).path
        if r !== run {
            let bgAct = cd_action(page)
            if bgAct == 1 { sendTask(r) }
            else if bgAct == 3 { stopRun(r) }
        }
        if FeatureFlags.anyExtraModel && cd_budget_exceeded(page) == 0 {
            while true {
                let j = cd_agent_next(page)
                if j < 0 { break }
                let one = CoderWorker()
                if FeatureFlags.isOn(.modelsClaude) && cd_agent_claude(page, j) != 0 { one.program = "claudeworker" }
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
        checkRunNotifications(r)
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
        r.stopRequestedAt = 0
        r.stopGen &+= 1
        let shown = r === run
        let needsRecall: Bool
        if shown {
            configureWorkerEnv(r, isForeman: isForemanShown)
            syncSkillsNote(for: r.page, isForeman: isForemanShown)
            needsRecall = recallDue
            if needsRecall { recallDue = false }
        } else {
            if !r.worker.alive {
                configureWorkerEnv(r, isForeman: false)
                syncSkillsNote(for: r.page, isForeman: false)
                needsRecall = true
            } else {
                r.worker.folder = r.folder.path
                needsRecall = false
            }
        }
        let useAgy = cd_model_agy(r.page) != 0
        #if OA_FULL
        let useInkling = !useAgy && (cd_model_inkling(r.page) != 0 || !FeatureFlags.anyExtraModel)
        let useHybrid = FeatureFlags.isOn(.modelsExecutor) && cd_model_hybrid(r.page) != 0
        let provKind = cd_model_provider(r.page)
        // Codex, Cursor, an ACP agent and Claude Code sign in on their own: no Gemini sign-in is needed
        let useExternal = FeatureFlags.anyExtraModel && (provKind == ExternalProviders.providerCodex || provKind == ExternalProviders.providerCursor || provKind == ExternalProviders.providerAcp || provKind == ExternalProviders.providerClaude)
        // a key set in Settings, or what Microcoder signs in with on its own (Vertex AI from a service
        // account or ADC, a Gemini API key in ~/work/.secrets or the environment: mcworker --auth)
        let cloudOk = FeatureFlags.isOn(.modelsGemini) && (CoderGemini.saved != nil || CoderProbes.geminiReady || (FeatureFlags.isOn(.account) && MacAccount.shared.loggedIn))
        if !useAgy && !useInkling && !useHybrid && !useExternal && !cloudOk {
            cd_failed(r.page, ProviderInstancesStore.shared.hasLoadedAntigravityModels
                ? "No Gemini sign-in found: set up Vertex AI (a Google Cloud service account) or add a Gemini API key in Settings → Models, or switch to an Antigravity model."
                : "No Gemini sign-in found: set up Vertex AI (a Google Cloud service account) or add a Gemini API key in Settings → Models, or sign into Antigravity in Settings → Providers.")
            wake()
            return
        }
        #else
        let useInkling = !useAgy
        let useHybrid = false
        #endif
        if useInkling || useHybrid {
            #if OA_FULL
            if useHybrid {
                let snap = ExecutorModelStore.scan()
                if !snap.selectable {
                    syncExecutorModels()
                    cd_failed(r.page, "Hybrid Executor refused: \(snap.status) (\(snap.lineage)). Open Settings → Models → Executor Models & Weight Modules.")
                    wake()
                    return
                }
            } else if !LocalModelStore.isSelectable {
                let why: String
                if !LocalModelStore.isAppleSilicon {
                    why = "Inkling 2 Mini requires an Apple Silicon Mac (macOS arm64)."
                } else if !LocalModelStore.hardwareSupported {
                    why = "Inkling 2 Mini requires at least 32 GB unified memory (\(LocalModelStore.physicalMemoryGB) GB detected)."
                } else if LocalModelStore.isVerifying {
                    why = "Inkling 2 Mini is verifying its SHA-256 checksum. Please wait a moment."
                } else if LocalModelStore.isDownloading {
                    why = "Inkling 2 Mini is still downloading (\(LocalModelStore.statusSummary()))."
                } else {
                    why = "Inkling 2 Mini is not ready (\(LocalModelStore.statusSummary())). Download or verify it in Settings → Models → Local Model."
                }
                cd_failed(r.page, why)
                if shown && LocalModelStore.hardwareSupported && !LocalModelStore.isDownloading && !LocalModelStore.isVerifying { manageLocalModels() }
                wake()
                return
            }
            #else
            if !LocalModelStore.isSelectable {
                let why: String
                if !LocalModelStore.isAppleSilicon {
                    why = "Inkling 2 Mini requires an Apple Silicon Mac (macOS arm64)."
                } else if !LocalModelStore.hardwareSupported {
                    why = "Inkling 2 Mini requires at least 32 GB unified memory (\(LocalModelStore.physicalMemoryGB) GB detected)."
                } else if LocalModelStore.isVerifying {
                    why = "Inkling 2 Mini is verifying its SHA-256 checksum. Please wait a moment."
                } else if LocalModelStore.isDownloading {
                    why = "Inkling 2 Mini is still downloading (\(LocalModelStore.statusSummary()))."
                } else {
                    why = "Inkling 2 Mini is not ready (\(LocalModelStore.statusSummary())). Download or verify it in Settings → Models → Local Model."
                }
                cd_failed(r.page, why)
                if shown && LocalModelStore.hardwareSupported && !LocalModelStore.isDownloading && !LocalModelStore.isVerifying { manageLocalModels() }
                wake()
                return
            }
            #endif
            r.startingWorker = true
            let gen = r.startGen
            if !LocalModelServer.shared.isReady {
                cd_turn_loading_model(r.page, 1)
                wake()
            }
            LocalModelServer.shared.ensureRunning(onLoading: { [weak self, weak r] in
                guard let self, let r, r.startGen == gen, cd_running(r.page) != 0 else { return }
                cd_turn_loading_model(r.page, 1)
                self.wake()
            }) { [weak self, weak r] err in
                guard let self, let r, r.startGen == gen else { return }
                r.startingWorker = false
                guard cd_running(r.page) != 0 else {
                    cd_turn_loading_model(r.page, 0)
                    return
                }
                cd_turn_loading_model(r.page, 0)
                if let err {
                    cd_failed(r.page, err)
                    self.wake()
                    return
                }
                self.wake()
                let latestURL = LocalModelServer.baseURL
                var retryRecall = needsRecall
                if r.worker.extraEnv["PSIONIC_BASE_URL"] != latestURL {
                    r.worker.extraEnv["PSIONIC_BASE_URL"] = latestURL
                    if r.worker.alive {
                        r.worker.stop(forget: false)
                        retryRecall = true
                    }
                }
                self.launchWorker(r, useAgy: false, needsRecall: retryRecall)
            }
            return
        }
        launchWorker(r, useAgy: useAgy, needsRecall: needsRecall)
    }
    private func launchWorker(_ r: CoderRun, useAgy: Bool, needsRecall: Bool) {
        #if OA_FULL
        let resolved = resolveProviderWorker(r)
        #else
        let resolved: (program: String, providerKind: Int32, handoffJSON: String?) = useAgy ? ("agyworker", 1, nil) : ("inky", 2, nil)
        #endif
        let wantProgram = resolved.program
        let switchedProvider = r.lastProviderKind >= 0 && r.lastProviderKind != resolved.providerKind
        let switchedProgram = r.worker.program != wantProgram || switchedProvider
        if switchedProgram {
            if r.worker.alive { r.worker.stop(forget: false) }
            r.worker.program = wantProgram
        }
        r.lastProviderKind = resolved.providerKind
        let freshSession = cd_fresh_session(r.page) != 0
        let sendRecall = needsRecall || switchedProgram || freshSession
        let handoffJSON = resolved.handoffJSON
        r.startingWorker = true
        let gen = r.startGen
        r.worker.startAsync { [weak self, weak r] ok in
            guard let self, let r else { return }
            guard r.startGen == gen else {
                if ok { r.worker.stop() }
                return
            }
            r.startingWorker = false
            let stillShown = r === self.run
            let stillBackground = !stillShown && r.chat != nil && self.runs[r.chat!] === r
            guard stillShown || stillBackground else {
                if ok { r.worker.stop() }
                return
            }
            guard cd_running(r.page) != 0 else { return }
            guard ok else {
                if r.worker.failure == CoderWorker.runLimitNote { self.sweepRuns() }   // the next send finds the slots it frees
                let fallbackErr = r.worker.program == "claudeworker" ? "Claude could not start." : r.worker.program == "agyworker" ? "Antigravity could not start." : r.worker.program == "inky" ? "Local agent worker could not start." : "Microcoder could not start."
                cd_failed(r.page, r.worker.failure ?? fallbackErr)
                self.wake()
                return
            }
            var out = [CChar](repeating: 0, count: 262144)
            let cwd = (stillShown ? self.folder : r.folder).path
            let from = r.worker.offset
            if freshSession {
                let n = cd_forget_line(&out, Int32(out.count))
                r.worker.write(out, count: Int(n))
            } else if let handoffJSON, !handoffJSON.isEmpty {
                let bytes = Array(handoffJSON.utf8CString)
                r.worker.write(bytes, count: max(0, bytes.count - 1))
            } else if sendRecall {
                // an opened chat's turns (or after switching workers mid-thread), or nothing to remember when it has none that ended
                var n = cd_recall_line(r.page, &out, Int32(out.count))
                if n == 0 { n = cd_forget_line(&out, Int32(out.count)) }
                r.worker.write(out, count: Int(n))
            }
            let n = cd_task(r.page, r.session, cwd, &out, Int32(out.count))
            r.worker.write(out, count: Int(n))
            self.beginTurn(r, from: from)
            #if OA_FULL
            self.captureBaseCheckpointIfNeeded(for: r)
            #endif
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
            cd_flags_set(back, FeatureFlags.mask)
            syncLocalState(on: back)
            syncAntigravityModels(on: back)
            #if OA_FULL
            if FeatureFlags.isOn(.modelsOpenrouter) { coderApplyOpenRouter(back) }
            if FeatureFlags.isOn(.modelsClaude) { coderApplyClaude(back) } else { cd_claude(back, 0) }
            if FeatureFlags.isOn(.modelsExternal) { syncExternalProviders(on: back) }
            cd_cloud_authed_set(back, (FeatureFlags.isOn(.account) && MacAccount.shared.loggedIn) ? 1 : 0)
            cd_settings_gemini(back, FeatureFlags.isOn(.modelsGemini) ? (CoderGemini.saved.map { "Ends " + String($0.suffix(4)) } ?? "") : "")
            #else
            cd_claude(back, 0)
            cd_cloud_authed_set(back, 0)
            cd_settings_gemini(back, "")
            #endif
            CoderProbes.apply(back)
            syncSkillsNote(for: back, isForeman: FeatureFlags.isOn(.foreman) && t.chat == Foreman.chatID)
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
            r.lastGoalStatus = cd_goal_status(back)
            r.lastPermitOpen = cd_permit_open(back) != 0
            if r.lastPermitOpen { needsAttention.insert(t.chat) }
            if t.chat != chat { unread.insert(t.chat) }
            named.insert(t.run); named.formUnion(r.turn?.agents ?? [])
            pump(r)
        }
        if let live = daemon.listRuns() {
            for one in live where !named.contains(one.run_id) && !CoderWorker.isPhoneRun(one.run_id) {
                let id = one.run_id
                // a run mid-reply is never stopped here: a lost turn file must not cost the work (#13810)
                if one.isLive && !CoderWorker.runIdle(id) { CoderWorker.logStop(id, "left running at launch: busy, though no kept message names it"); continue }
                if one.isLive { CoderWorker.logStop(id, "stopped at launch: idle, no kept message names it"); daemon.stopRun(id) { DispatchQueue.main.asyncAfter(deadline: .now() + 4) { daemon.forgetRun(id) } } } else { daemon.forgetRun(id) }
            }
        }
        syncPhoneRuns()
        cd_chats(page, chatLines)
        #if OA_FULL
        syncForemanUI()
        #endif
        updateDockBadge()
    }
    /// Every daemon run this Coder holds: each chat's worker and sub-agents, in view or out, and the
    /// runs its kept messages name.
    var heldRuns: Set<String> {
        var held = Set<String>()
        for r in [run] + Array(runs.values) {
            if let id = r.worker.runID { held.insert(id) }
            for one in r.agents.values { if let id = one.runID { held.insert(id) } }
            if let t = r.turn { held.insert(t.run); held.formUnion(t.agents) }
        }
        return held
    }
    static let sweepEvery: CFTimeInterval = 60
    static let sweepAge = 60   // seconds a run is left alone after it starts: one still starting is not yet held
    /// A live run no chat holds is stopped and forgotten while Coder runs, not only at launch
    /// (reattach): a worker waits on stdin after its turn, so a run whose StopRun was lost would
    /// otherwise keep one of the daemon's slots until every send is refused (#13810).
    func sweepRuns() {
        sweptAt = CACurrentMediaTime()
        guard let daemon = CoderWorker.daemon, daemon.ownsRuns, !sweeping else { return }
        sweeping = true
        DispatchQueue.global(qos: .utility).async { [weak self] in
            let live = daemon.listRuns()
            DispatchQueue.main.async {
                guard let self else { return }
                self.sweeping = false
                guard let live else { return }
                let held = self.heldRuns, now = Int(Date().timeIntervalSince1970)
                for id in CoderWindow.orphans(live, held: held, now: now) where !CoderWorker.isPhoneRun(id) && CoderWorker.runIdle(id) {
                    CoderWorker.logStop(id, "stopped by the sweep: idle, no chat holds it")
                    daemon.stopRun(id) { DispatchQueue.main.asyncAfter(deadline: .now() + 4) { daemon.forgetRun(id) } }
                }
            }
        }
    }
    /// The live runs to stop: none a chat holds, and none started less than sweepAge seconds ago
    /// (sweepRuns also leaves any run mid-reply alone: only an idle one is stopped).
    static func orphans(_ live: [DaemonRun], held: Set<String>, now: Int) -> [String] {
        live.filter { $0.isLive && !held.contains($0.run_id) && now - ($0.started ?? now) >= sweepAge }.map { $0.run_id }
    }

    // ---- Phone chat sync (host-phone.coax -> CoderStore sidebar + live transcript) ----

    private struct PhoneRunSnapshot {
        let runID: String
        let uuid: String
        let stamp: PhoneRunStamp
        let title: String
        let cwd: String
        let savedModel: String
        let effort: String
        let base: Int
        let isBusy: Bool
        let turns: [[String: Any]]
    }

    /// Scans `~/.openagents/coder/host/runs/*.phone` off the main thread and mirrors new or updated
    /// phone-originated chats into `CoderStore` so they appear in Desktop's sidebar live and persist.
    func syncPhoneRuns() {
        phoneSyncedAt = CACurrentMediaTime()
        guard let daemon = CoderWorker.daemon, !phoneSyncing else { return }
        phoneSyncing = true
        let runsDir = daemon.runsDir
        let prevStamps = phoneRunStamps
        let prevBusy = phoneBusyChats
        DispatchQueue.global(qos: .utility).async { [weak self] in
            let fm = FileManager.default
            let names = (try? fm.contentsOfDirectory(atPath: runsDir.path)) ?? []
            var updates: [PhoneRunSnapshot] = []
            var nextBusy = Set<String>()
            var seenRunIDs = Set<String>()
            for name in names where name.hasSuffix(".phone") && name.count == 38 {
                let runID = String(name.prefix(32))
                guard let uuid = CoderWorker.phoneChatUUID(from: runID) else { continue }
                seenRunIDs.insert(runID)
                let stateURL = runsDir.appendingPathComponent(runID + ".state")
                guard let stateAttrs = try? fm.attributesOfItem(atPath: stateURL.path) else { continue }
                let inURL = runsDir.appendingPathComponent(runID + ".in")
                let outURL = runsDir.appendingPathComponent(runID + ".out")
                let phoneURL = runsDir.appendingPathComponent(runID + ".phone")
                let inAttrs = try? fm.attributesOfItem(atPath: inURL.path)
                let outAttrs = try? fm.attributesOfItem(atPath: outURL.path)
                let phoneAttrs = try? fm.attributesOfItem(atPath: phoneURL.path)
                let stamp = PhoneRunStamp(
                    stateMtime: (stateAttrs[.modificationDate] as? Date)?.timeIntervalSince1970 ?? 0,
                    inSize: (inAttrs?[.size] as? NSNumber)?.uint64Value ?? 0,
                    outSize: (outAttrs?[.size] as? NSNumber)?.uint64Value ?? 0,
                    outMtime: (outAttrs?[.modificationDate] as? Date)?.timeIntervalSince1970 ?? 0,
                    phoneMtime: (phoneAttrs?[.modificationDate] as? Date)?.timeIntervalSince1970 ?? 0
                )
                if prevStamps[runID] == stamp {
                    if prevBusy.contains(uuid) { nextBusy.insert(uuid) }
                    continue
                }
                if let snap = CoderWindow.parsePhoneRun(runID: runID, uuid: uuid, runsDir: runsDir, stamp: stamp) {
                    if snap.isBusy { nextBusy.insert(uuid) }
                    updates.append(snap)
                }
            }
            DispatchQueue.main.async { [weak self] in
                guard let self else { return }
                self.phoneSyncing = false
                for key in Array(self.phoneRunStamps.keys) where !seenRunIDs.contains(key) {
                    self.phoneRunStamps.removeValue(forKey: key)
                }
                var sidebarChanged = (self.phoneBusyChats != nextBusy)
                self.phoneBusyChats = nextBusy
                for snap in updates {
                    if self.store.isPhoneRunDeleted(snap.runID) {
                        self.phoneRunStamps[snap.runID] = snap.stamp
                        continue
                    }
                    // Never overwrite while Desktop itself is running a turn in this chat.
                    if self.runs[snap.uuid] != nil || (self.chat == snap.uuid && cd_running(self.page) != 0) {
                        continue
                    }
                    let existingMeta = self.store.meta(snap.uuid)
                    var existingTurns: [[String: Any]] = []
                    var existingCost = 0
                    var existingBudget = 50_000_000
                    if existingMeta != nil, let raw = self.store.text(snap.uuid),
                       let data = raw.data(using: .utf8),
                       let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
                        existingTurns = (obj["turns"] as? [[String: Any]]) ?? []
                        existingCost = (obj["cost_usd_micros"] as? Int) ?? 0
                        existingBudget = (obj["budget_usd_micros"] as? Int) ?? 50_000_000
                    }
                    // If Desktop already has more turns than this phone run, do not overwrite Desktop's newer turns.
                    if snap.base + snap.turns.count < existingTurns.count {
                        self.phoneRunStamps[snap.runID] = snap.stamp
                        continue
                    }
                    // On initial launch scan, avoid bumping an already-synced idle phone chat to the top of the sidebar.
                    let isFirstScan = prevStamps[snap.runID] == nil
                    if isFirstScan, let meta = existingMeta, !snap.isBusy,
                       snap.base + snap.turns.count <= existingTurns.count,
                       snap.stamp.outMtime <= meta.updated + 1.0 {
                        self.phoneRunStamps[snap.runID] = snap.stamp
                        continue
                    }
                    let prefixCount = min(snap.base, existingTurns.count)
                    let mergedTurns = Array(existingTurns.prefix(prefixCount)) + snap.turns
                    guard !mergedTurns.isEmpty else {
                        self.phoneRunStamps[snap.runID] = snap.stamp
                        continue
                    }
                    var doc: [String: Any] = [
                        "v": 1,
                        "cost_usd_micros": existingCost,
                        "budget_usd_micros": existingBudget,
                        "model": snap.savedModel,
                        "turns": mergedTurns
                    ]
                    if !snap.effort.isEmpty { doc["effort"] = snap.effort }
                    guard let jsonData = try? JSONSerialization.data(withJSONObject: doc, options: []),
                          let jsonText = String(data: jsonData, encoding: .utf8) else { continue }
                    let isNewChat = (existingMeta == nil)
                    let gainedTurn = mergedTurns.count > existingTurns.count
                    self.store.keep(id: snap.uuid, title: snap.title, folder: snap.cwd, text: jsonText)
                    self.phoneRunStamps[snap.runID] = snap.stamp
                    sidebarChanged = true
                    if self.chat == snap.uuid && cd_running(self.page) == 0 {
                        if cd_load(self.page, jsonText) != 0 {
                            self.recallDue = true
                            self.wake()
                        }
                    } else if isNewChat || gainedTurn || !snap.isBusy {
                        if self.chat != snap.uuid { self.unread.insert(snap.uuid) }
                    }
                }
                if sidebarChanged {
                    cd_chats(self.page, self.chatLines)
                    self.updateDockBadge()
                    self.wake()
                }
            }
        }
    }

    /// Parses one phone run's `.phone`, `.state`, `.in`, and `.out` files into Desktop's saved chat turn schema.
    private static func parsePhoneRun(runID: String, uuid: String, runsDir: URL, stamp: PhoneRunStamp) -> PhoneRunSnapshot? {
        guard let phoneData = try? Data(contentsOf: runsDir.appendingPathComponent(runID + ".phone")),
              let phoneObj = try? JSONSerialization.jsonObject(with: phoneData) as? [String: Any] else { return nil }
        let engine = (phoneObj["engine"] as? String) ?? ""
        let rawModel = (phoneObj["model"] as? String) ?? ""
        let effort = (phoneObj["effort"] as? String) ?? ""
        let base = max(0, (phoneObj["base"] as? Int) ?? 0)
        let savedModel: String
        if engine == "antigravity" {
            let m = rawModel.isEmpty ? "gemini-4-neon" : rawModel
            savedModel = m.hasPrefix("agy:") ? m : "agy:" + m
        } else if engine == "inkling" {
            savedModel = "inkling-2-mini"
        } else {
            savedModel = rawModel.isEmpty ? "gemini-4-neon" : rawModel
        }

        let stateData = try? Data(contentsOf: runsDir.appendingPathComponent(runID + ".state"))
        let stateObj = stateData.flatMap { try? JSONSerialization.jsonObject(with: $0) as? [String: Any] }
        let runState = (stateObj?["state"] as? String) ?? ""
        let isRunning = (runState == "running")

        // Parse task prompts and cwd from ID.in.
        var tasks: [String] = []
        var cwd = ""
        if let inText = try? String(contentsOf: runsDir.appendingPathComponent(runID + ".in"), encoding: .utf8) {
            for rawLine in inText.split(separator: "\n", omittingEmptySubsequences: true) {
                guard let lineData = String(rawLine).data(using: .utf8),
                      let obj = try? JSONSerialization.jsonObject(with: lineData) as? [String: Any],
                      let task = obj["task"] as? String else { continue }
                tasks.append(task)
                if cwd.isEmpty, let c = obj["cwd"] as? String, !c.isEmpty { cwd = c }
            }
        }
        if cwd.isEmpty { cwd = FileManager.default.homeDirectoryForCurrentUser.path }

        // Parse mcworker-wire lines from ID.out (same semantics as host-desktop.coax dk_session).
        let outText = (try? String(contentsOf: runsDir.appendingPathComponent(runID + ".out"), encoding: .utf8)) ?? ""
        let outLines = outText.split(separator: "\n", omittingEmptySubsequences: true)
        var parsedLines: [[String: Any]] = []
        parsedLines.reserveCapacity(outLines.count)
        var wordlessTurns = 0
        for rawLine in outLines {
            guard let lineData = String(rawLine).data(using: .utf8),
                  let obj = try? JSONSerialization.jsonObject(with: lineData) as? [String: Any] else { continue }
            parsedLines.append(obj)
            if (obj["kind"] as? String) == "turn",
               (obj["source"] as? String) == "message",
               ((obj["text"] as? String) ?? "").isEmpty {
                wordlessTurns += 1
            }
        }

        // Align wordless message turns 1-to-1 with tasks in ID.in (oldest-first since whole files are read;
        // if more turns exist than tasks, skip the oldest unmatched turns).
        var skipWordless = max(0, wordlessTurns - tasks.count)
        var nextTaskIdx = 0

        struct WorkingTurn {
            var ask: String
            var reply: String = ""
            var state: Int = 0 // 0=running, 1=done, 2=failed, 3=stopped
            var note: String = ""
            var steers: [String] = []
            var steerById: [String: String] = [:]
            var cmds: [[String: Any]] = []
            var openAskIdx: Int? = nil
            var ended: Bool = false

            mutating func flushInterleavedReply() {
                let trimmed = reply.trimmingCharacters(in: .whitespacesAndNewlines)
                if !trimmed.isEmpty {
                    cmds.append(["text": "", "out": reply, "read": false, "failed": false, "said": true, "took": 0])
                    reply = ""
                }
            }
            func toDict(model: String) -> [String: Any] {
                var d: [String: Any] = [
                    "ask": ask,
                    "reply": reply,
                    "cut": false,
                    // Persist running phone turns as state=1 so opening while streaming does not show "Stopped"
                    "state": ended ? state : 1,
                    "model": model,
                    "note": note,
                    "more": 0,
                    "earlier": 0,
                    "cmds": cmds,
                    "agents": []
                ]
                if !steers.isEmpty { d["steers"] = steers }
                return d
            }
        }

        var working: [WorkingTurn] = []
        var openTurn = false

        for obj in parsedLines {
            guard let kind = obj["kind"] as? String else { continue }
            switch kind {
            case "turn":
                if !working.isEmpty && !working[working.count - 1].ended {
                    working[working.count - 1].ended = true
                    working[working.count - 1].state = 1
                }
                let rawText = (obj["text"] as? String) ?? ""
                let source = (obj["source"] as? String) ?? ""
                let ask: String
                if rawText.isEmpty && source == "message" {
                    if skipWordless > 0 {
                        skipWordless -= 1
                        ask = "(follow-up)"
                    } else if nextTaskIdx < tasks.count {
                        ask = tasks[nextTaskIdx]
                        nextTaskIdx += 1
                    } else {
                        ask = "(follow-up)"
                    }
                } else {
                    ask = rawText.isEmpty ? "(follow-up)" : rawText
                }
                working.append(WorkingTurn(ask: ask))
                openTurn = true
            case "thought":
                guard !working.isEmpty, let t = obj["text"] as? String, !t.isEmpty else { continue }
                let idx = working.count - 1
                if let last = working[idx].cmds.last, (last["thought"] as? Bool) == true {
                    let prevOut = (last["out"] as? String) ?? ""
                    working[idx].cmds[working[idx].cmds.count - 1]["out"] = prevOut + t
                } else {
                    let head = CoderStore.cleanTitle(t.split(separator: "\n", maxSplits: 1).first.map(String.init) ?? t)
                    working[idx].cmds.append([
                        "text": head.isEmpty ? "Thinking" : head,
                        "out": t,
                        "read": false,
                        "failed": false,
                        "thought": true,
                        "took": 0
                    ])
                }
            case "reply":
                guard !working.isEmpty, let t = obj["text"] as? String else { continue }
                working[working.count - 1].reply += t
            case "ask":
                guard !working.isEmpty, let cmd = obj["command"] as? String else { continue }
                let idx = working.count - 1
                working[idx].flushInterleavedReply()
                working[idx].cmds.append(["text": cmd, "out": "", "read": false, "failed": false, "took": 0])
                working[idx].openAskIdx = working[idx].cmds.count - 1
            case "command":
                guard !working.isEmpty, let cmd = obj["command"] as? String else { continue }
                let idx = working.count - 1
                let out = (obj["output"] as? String) ?? ""
                let exitCode = (obj["exit"] as? Int) ?? 0
                let timedOut = (obj["timed_out"] as? Bool) ?? false
                let failed = (exitCode != 0) || timedOut
                if let askIdx = working[idx].openAskIdx,
                   askIdx < working[idx].cmds.count,
                   (working[idx].cmds[askIdx]["text"] as? String) == cmd {
                    working[idx].cmds[askIdx]["out"] = out
                    working[idx].cmds[askIdx]["failed"] = failed
                    working[idx].openAskIdx = nil
                } else {
                    working[idx].flushInterleavedReply()
                    working[idx].cmds.append(["text": cmd, "out": out, "read": false, "failed": failed, "took": 0])
                    working[idx].openAskIdx = nil
                }
            case "view":
                guard !working.isEmpty, let path = obj["path"] as? String else { continue }
                let idx = working.count - 1
                let found = (obj["found"] as? Bool) ?? true
                working[idx].flushInterleavedReply()
                working[idx].cmds.append(["text": path, "out": found ? "" : "Not found", "read": true, "failed": !found, "took": 0])
            case "steered":
                guard !working.isEmpty else { continue }
                let idx = working.count - 1
                let status = (obj["status"] as? String) ?? ""
                let sid = (obj["id"] as? String) ?? ""
                if status == "accepted", let t = obj["text"] as? String, !sid.isEmpty, !t.isEmpty {
                    working[idx].steerById[sid] = t
                } else if status == "applied", !sid.isEmpty, let t = working[idx].steerById[sid] {
                    working[idx].steers.append(t)
                }
            case "ended":
                guard !working.isEmpty else { continue }
                let idx = working.count - 1
                let ending = (obj["ending"] as? String) ?? ""
                let endText = (obj["text"] as? String) ?? ""
                working[idx].ended = true
                openTurn = false
                if ending == "finished" || ending == "asked" {
                    if working[idx].reply.isEmpty && !endText.isEmpty { working[idx].reply = endText }
                    working[idx].state = 1
                } else if ending == "stopped" {
                    working[idx].state = 3
                } else {
                    working[idx].state = 2
                    working[idx].note = endText.isEmpty ? "The turn ended early." : endText
                }
            default:
                break
            }
        }

        // If the phone just started or sent a follow-up and the worker has not emitted its turn line yet,
        // synthesize the pending turn so the chat/follow-up appears in Desktop's sidebar immediately.
        var isBusy = isRunning && openTurn
        if isRunning && !openTurn && nextTaskIdx < tasks.count {
            while nextTaskIdx < tasks.count {
                let pendingTask = tasks[nextTaskIdx]
                nextTaskIdx += 1
                if !pendingTask.isEmpty {
                    working.append(WorkingTurn(ask: pendingTask))
                    isBusy = true
                }
            }
        } else if !isRunning && !working.isEmpty && !working[working.count - 1].ended {
            working[working.count - 1].ended = true
            working[working.count - 1].state = 3
        }

        guard !working.isEmpty else { return nil }
        let titleRaw = working.first?.ask ?? tasks.first ?? "New chat"
        let cleanTitle = CoderStore.cleanTitle(titleRaw)
        let title = cleanTitle.isEmpty ? "New chat" : cleanTitle
        let turns = working.map { $0.toDict(model: savedModel) }
        return PhoneRunSnapshot(
            runID: runID,
            uuid: uuid,
            stamp: stamp,
            title: title,
            cwd: cwd,
            savedModel: savedModel,
            effort: effort,
            base: base,
            isBusy: isBusy,
            turns: turns
        )
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
    /// True when `name` is an auto-generated internal worktree branch (e.g. `coder/<uuid>`).
    private static func isAutoWorktreeBranch(_ name: String) -> Bool {
        guard name.hasPrefix("coder/") else { return false }
        let suffix = name.dropFirst(6)
        return suffix.count >= 16 && suffix.allSatisfy { $0.isHexDigit || $0 == "-" }
    }
    /// Fast zero-subprocess `.git/HEAD` reader for sidebar card branch labels.
    private static func quickGitBranch(at dir: String) -> String {
        guard !dir.isEmpty else { return "" }
        let gitPath = (dir as NSString).appendingPathComponent(".git")
        var isDir: ObjCBool = false
        guard FileManager.default.fileExists(atPath: gitPath, isDirectory: &isDir) else { return "" }
        let headPath: String
        if isDir.boolValue {
            headPath = (gitPath as NSString).appendingPathComponent("HEAD")
        } else if let ptr = try? String(contentsOfFile: gitPath, encoding: .utf8),
                  ptr.hasPrefix("gitdir:") {
            let rel = ptr.dropFirst(7).trimmingCharacters(in: .whitespacesAndNewlines)
            let resolved = rel.hasPrefix("/") ? rel : (dir as NSString).appendingPathComponent(rel)
            headPath = (resolved as NSString).appendingPathComponent("HEAD")
        } else {
            return ""
        }
        guard let head = try? String(contentsOfFile: headPath, encoding: .utf8) else { return "" }
        let line = head.trimmingCharacters(in: .whitespacesAndNewlines)
        if line.hasPrefix("ref: refs/heads/") {
            let b = String(line.dropFirst(16))
            return isAutoWorktreeBranch(b) ? "" : b
        }
        if line.count >= 7 { return String(line.prefix(7)) }
        return ""
    }
    /// The list as the page reads it: one chat a line, its id, a tab, its title, and optional tabs
    /// for busy, pinned, managed, unread, archived, snoozed_until_ms, folder, updated_ms,
    /// branch, and harness (#13875).
    var chatLines: String {
        cd_sb_now_ms(page, Int64(Date().timeIntervalSince1970 * 1000))
        var branchCache: [String: String] = [:]
        return store.orderedList.map { chat in
            let isBusy = runs[chat.id] != nil || phoneBusyChats.contains(chat.id)
            let isPinned = chat.pinned != nil
            let isManaged = chat.managedBy == Foreman.managedTag
            let isUnread = unread.contains(chat.id)
            let isArchived = store.isArchived(chat.id)
            let snoozeMs = store.snoozedUntil(chat.id)
            let dir = store.effectiveFolder(chat)
                .replacingOccurrences(of: "\t", with: " ")
                .replacingOccurrences(of: "\n", with: "")
            let updatedMs = Int64(chat.updated * 1000)
            let branch: String
            if let wb = chat.worktreeBranch, !wb.isEmpty, !CoderWindow.isAutoWorktreeBranch(wb) {
                branch = wb
            } else if let cached = branchCache[dir] {
                branch = cached
            } else {
                let b = CoderWindow.quickGitBranch(at: dir)
                branchCache[dir] = b
                branch = b
            }
            let activeRun: CoderRun? = runs[chat.id] ?? (chat.id == self.chat && (cd_running(page) != 0 || run.worker.alive) ? run : nil)
            let prog = activeRun?.worker.program ?? ""
            let codexEnv = activeRun?.worker.extraEnv["CODER_PROVIDER"] == "codex"
            let activeHarness: String? = prog == "claudeworker" ? "claude"
                : prog == "agyworker" ? "agy"
                : (prog == "codex" || codexEnv) ? "codex"
                : prog == "inky" ? "psionic"
                : nil
            let harness = isManaged ? "foreman"
                : (activeHarness ?? store.harness(chat.id))
            let cleanBranch = branch.replacingOccurrences(of: "\t", with: " ").replacingOccurrences(of: "\n", with: "")
            return chat.id + "\t" + chat.title + "\t" + (isBusy ? "1" : "0") + "\t" + (isPinned ? "1" : "0") + "\t" + (isManaged ? "1" : "0") + "\t" + (isUnread ? "1" : "0") + "\t" + (isArchived ? "1" : "0") + "\t" + String(snoozeMs) + "\t" + dir + "\t" + String(updatedMs) + "\t" + cleanBranch + "\t" + harness
        }.joined(separator: "\n")
    }
    var isForemanShown: Bool { chat == Foreman.chatID }
    func syncFromDaemon() {
        #if OA_FULL
        daemonClient.readForeman { [weak self] state in
            guard let self, let rawTier = state?.tier, let tier = ForemanTier(rawValue: rawTier) else { return }
            if self.foremanTier != tier {
                self.foremanTier = tier
                tier.save()
                self.syncForemanUI()
                self.wake()
            }
        }
        #endif
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
                if let archived = item.archived, self.store.isArchived(item.session_id) != archived {
                    self.store.setArchived(item.session_id, archived)
                    changed = true
                }
            }
            if changed {
                cd_chats(self.page, self.chatLines)
                self.wake()
            }
        }
    }
    #if OA_FULL
    func syncForemanUI() {
        let busy = runs[Foreman.chatID] != nil || (chat == Foreman.chatID && cd_running(page) != 0)
        cd_foreman(page, busy ? 1 : 0, Int32(foremanTier.rawValue))
    }
    #endif
    func syncSkillsNote(for pagePtr: UnsafeMutableRawPointer, isForeman: Bool) {
        #if OA_FULL
        let base = FeatureFlags.isOn(.mcp) ? coderSkillsNote(repoFolder: folder) : ""
        if isForeman && FeatureFlags.isOn(.foreman) {
            let note = Foreman.systemPrompt(tier: foremanTier, workspace: folder.path, chatCount: store.orderedList.count, runningCount: runningCount, baseSkills: base)
            cd_skills(pagePtr, note)
        } else {
            cd_skills(pagePtr, base)
        }
        #else
        cd_skills(pagePtr, "")
        #endif
    }
    func configureWorkerEnv(_ r: CoderRun, isForeman: Bool) {
        var env: [String: String] = [:]
        var instBuf = [CChar](repeating: 0, count: 128)
        let instN = cd_model_inst_id(r.page, &instBuf, Int32(instBuf.count))
        let pickedInstId = instN > 0 ? String(cString: instBuf) : ""
        if let inst = ProviderInstancesStore.shared.instance(id: pickedInstId) ?? ProviderInstancesStore.shared.activeAntigravityInstance() {
            let res = AntigravityRuntimeManager.shared.resolveBinary(for: inst)
            env = AntigravityProfileManager.workerEnv(for: inst, resolvedBinary: res.path)
        } else {
            env = CoderAntigravity.workerEnv()
        }
        #if OA_FULL
        if FeatureFlags.isOn(.mcp) {
            for (k, v) in McpServerStore.workerEnv() { env[k] = v }
        }
        if isForeman && FeatureFlags.isOn(.foreman) && foremanServer.port > 0 {
            env["CODER_FOREMAN_URL"] = foremanServer.baseURL
            env["CODER_FOREMAN_TOKEN"] = foremanServer.token
            env["CODER_FOREMAN_TIER"] = foremanTier.wireName
            if let res = Bundle.main.resourcePath {
                // Claude Code is found on the person's PATH, so a Foreman on Claude keeps it after the app's tools
                env["PATH"] = (FeatureFlags.isOn(.modelsClaude) && cd_model_claude(r.page) != 0) ? res + ":" + CoderWorker.path : res
            }
        }
        if FeatureFlags.isOn(.modelsExecutor) && cd_model_hybrid(r.page) != 0 {
            env["PSIONIC_BASE_URL"] = LocalModelServer.baseURL
            env["PSIONIC_MODEL"] = "hybrid-exec-v1"
        } else if cd_model_inkling(r.page) != 0 || (!FeatureFlags.anyExtraModel && cd_model_agy(r.page) == 0) {
            env["PSIONIC_BASE_URL"] = LocalModelServer.baseURL
            env["PSIONIC_MODEL"] = "inkling-2-mini"
        }
        // the simulator helper's oasim (Window+Simulator.swift): which chat asks, and the engine the card names
        if FeatureFlags.isOn(.simulator) {
            env["OA_SIM_CHAT"] = (r === run ? chat : r.chat) ?? r.simChat
            env["OA_SIM_ENGINE"] = r.worker.program == "claudeworker" ? "Claude" : r.worker.program == "agyworker" ? "Antigravity" : "Microcoder"
        }
        #else
        if cd_model_agy(r.page) == 0 {
            env["PSIONIC_BASE_URL"] = LocalModelServer.baseURL
            env["PSIONIC_MODEL"] = "inkling-2-mini"
        }
        #endif
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
        #if OA_FULL
        syncForemanUI()
        #endif
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
        cd_flags_set(back, FeatureFlags.mask)
        syncLocalState(on: back)
        syncAntigravityModels(on: back)
        #if OA_FULL
        if FeatureFlags.isOn(.modelsOpenrouter) { coderApplyOpenRouter(back) }
        #endif
        guard cd_take(back, page) != 0 else { cd_close(back); return false }
        let moved = run
        moved.page = back; moved.folder = folder; moved.chat = id
        runs[id] = moved
        run = CoderRun(page: page, folder: folder); wire(run)
        chat = nil; recallDue = false; cd_current(page, "")
        configureWorkerEnv(run, isForeman: false)
        syncSkillsNote(for: page, isForeman: false)
        cd_chats(page, chatLines)
        #if OA_FULL
        syncForemanUI()
        #endif
        wake()
        return true
    }
    /// A reply out of view ended: its chat is kept, its workers stop and its page goes.
    func finish(_ id: String) {
        guard let back = runs.removeValue(forKey: id) else { return }
        #if OA_FULL
        let cost = Int(cd_cost_usd_micros(back.page))
        captureTurnCheckpoint(for: back)
        #endif
        back.stop(); cd_close(back.page)
        #if OA_FULL
        _ = foremanEvents.emit(type: "chat.finished", chatID: id, data: [
            "title": store.title(id) ?? "Chat",
            "cost_usd_micros": cost
        ])
        #endif
        if id != chat { unread.insert(id) }
        needsAttention.remove(id)
        notifyTurnCompleted(chatID: id, isActiveChat: id == chat)
        cd_chats(page, chatLines)
        #if OA_FULL
        syncForemanUI()
        #endif
        wake()
    }
}
