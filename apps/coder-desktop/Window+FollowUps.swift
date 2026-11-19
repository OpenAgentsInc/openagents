// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
// Follow-ups (GitHub #13715, #13716, #13717): steer a running reply, queue messages for after it, interrupt it,
// and what the tray over the composer asks for.
import AppKit

/// The worker owns a chat's queue and its running turn's steers (coder/tui/mcworker.coax): it keeps
/// the queue in ~/.openagents/coder/sessions/CHAT.queue.json, starts the next message itself when
/// a turn ends well, and pauses the queue after an error or an interrupt. So the queue goes on while
/// Coder is closed (the daemon runs the worker) and survives a restart. The host only writes lines
/// (page/followups.coax makes them) and reads the state the worker last sent back from the page.
/// Each steer and queued message has an id made here, so a line written twice changes nothing.
extension CoderWindow {
    /// The chat id a run's queue is kept under, once the chat is kept.
    func queueKey(_ r: CoderRun) -> String? { r === run ? chat : r.chat }
    /// Binds the worker's queue to its chat's file (idempotent): after a message's task, and before
    /// any queue line. A queue written before the chat was kept moves into that file.
    func bindQueue(_ r: CoderRun) {
        guard r.worker.alive, let key = queueKey(r) else { return }
        var out = [CChar](repeating: 0, count: 4096)
        let n = cd_queue_line(r.page, "open", "", 0, 0, key, &out, Int32(out.count))
        r.worker.write(out, count: Int(n))
    }
    /// Input for the running turn: it reaches the model between steps, without starting over. Its
    /// id, or nil when no turn of the worker runs (the page's steer statuses say what became of it).
    @discardableResult func steer(_ text: String, on which: CoderRun? = nil) -> String? {
        let r = which ?? run, id = UUID().uuidString.lowercased()
        guard r.worker.alive else { return nil }
        var out = [CChar](repeating: 0, count: 65536)
        let n = cd_steer_line(r.page, id, text, &out, Int32(out.count))
        guard n > 0 else { return nil }
        r.worker.write(out, count: Int(n))
        r.steered[id] = text
        wake()
        return id
    }
    /// A message for after the running turn, last in the queue. Its id, or nil when no worker runs
    /// (send it as a message then). The whole text stays here; the page keeps a preview.
    @discardableResult func enqueue(_ text: String, on which: CoderRun? = nil) -> String? {
        let r = which ?? run, id = UUID().uuidString.lowercased()
        guard r.worker.alive else { return nil }
        bindQueue(r)
        var out = [CChar](repeating: 0, count: 300_000)
        let n = cd_queue_message_line(r.page, "add", id, text, r.session, (r === run ? folder : r.folder).path, &out, Int32(out.count))
        guard n > 0 else { return nil }
        r.worker.write(out, count: Int(n))
        r.queued[id] = text
        wake()
        return id
    }
    /// A queued message's new text.
    func editQueued(_ id: String, _ text: String, on which: CoderRun? = nil) {
        let r = which ?? run
        guard r.worker.alive else { return }
        var out = [CChar](repeating: 0, count: 300_000)
        let n = cd_queue_message_line(r.page, "update", id, text, r.session, (r === run ? folder : r.folder).path, &out, Int32(out.count))
        if n > 0 { r.worker.write(out, count: Int(n)); r.queued[id] = text; wake() }
    }
    /// One queue op: delete, move (to), promote (now: stop the turn and start this one next),
    /// pause, resume, clear or list.
    func queueOp(_ op: String, _ id: String = "", to: Int = 0, now: Bool = false, on which: CoderRun? = nil) {
        let r = which ?? run
        guard r.worker.alive else { return }
        bindQueue(r)
        var out = [CChar](repeating: 0, count: 4096)
        let n = cd_queue_line(r.page, op, id, Int32(to), now ? 1 : 0, "", &out, Int32(out.count))
        guard n > 0 else { return }
        r.worker.write(out, count: Int(n))
        if op == "delete" { r.queued[id] = nil } else if op == "clear" { r.queued = [:] }
        wake()
    }
    /// Stops the running turn. With resubmit, the steers it had not taken in yet are sent as a new
    /// turn; without, they go to the queue's head and the queue pauses.
    func interrupt(resubmit: Bool, on which: CoderRun? = nil) {
        let r = which ?? run
        guard r.worker.alive else { return }
        var out = [CChar](repeating: 0, count: 4096)
        let n = cd_interrupt_line(r.page, resubmit ? 1 : 0, &out, Int32(out.count))
        r.worker.write(out, count: Int(n))
        for (_, one) in r.agents { var stop = [CChar](repeating: 0, count: 4096); let m = cd_stop_line(&stop, Int32(stop.count)); one.write(stop, count: Int(m)) }
        wake()
    }
    /// After a worker line: a turn the worker started on its own (the queue's head, or resubmitted
    /// steers) is kept like a sent message, so a reopened Coder attaches to it.
    func followUpBegan(_ r: CoderRun) {
        guard cd_followup_began(r.page) != 0, r.worker.isDaemonRun else { return }
        beginTurn(r, from: r.worker.offset)
    }

    // ------------------------------------------------------------ the tray and the composer (#13717)

    /// The text of queued item i: the whole text this Coder queued, else the page's preview.
    func queuedText(_ r: CoderRun, _ i: Int) -> (id: String, text: String)? {
        var out = [CChar](repeating: 0, count: 4096)
        guard cd_queue_item_id(r.page, Int32(i), &out, Int32(out.count)) > 0 else { return nil }
        let id = String(cString: out)
        if let whole = r.queued[id] { return (id, whole) }
        _ = cd_queue_item_text(r.page, Int32(i), &out, Int32(out.count))
        return (id, String(cString: out))
    }
    /// Steer i's id and text: the whole text this Coder sent, else the page's preview.
    func steerText(_ r: CoderRun, _ i: Int) -> (id: String, text: String)? {
        var out = [CChar](repeating: 0, count: 4096)
        guard cd_steer_id(r.page, Int32(i), &out, Int32(out.count)) > 0 else { return nil }
        let id = String(cString: out)
        if let whole = r.steered[id] { return (id, whole) }
        _ = cd_steer_text(r.page, Int32(i), &out, Int32(out.count))
        return (id, String(cString: out))
    }
    /// The composer's text, and the composer emptied.
    func takeDraft() -> String {
        let text = editor.string
        editor.synchronize { editor.string = "" }
        _ = cd_set_text(page, "")
        return text
    }
    func setDraft(_ text: String) {
        editor.synchronize { editor.string = text }
        _ = cd_set_text(page, text)
        window.makeFirstResponder(editor)
    }
    /// A worker for the shown chat's queue while no reply runs (a queue kept from before, paused):
    /// started if it is not, told the chat's turns if it was opened since, so a message the queue
    /// starts has them.
    @discardableResult func queueWorker() -> Bool {
        if !run.worker.alive {
            configureWorkerEnv(run, isForeman: isForemanShown)
            syncSkillsNote(for: page, isForeman: isForemanShown)
            guard run.worker.start() else { cd_failed(page, run.worker.failure ?? "Microcoder could not start."); return false }
        }
        if recallDue {
            recallDue = false
            var out = [CChar](repeating: 0, count: 262144)
            var n = cd_recall_line(page, &out, Int32(out.count))
            if n == 0 { n = cd_forget_line(&out, Int32(out.count)) }
            run.worker.write(out, count: Int(n))
        }
        return true
    }
    /// Command-Return (the File menu's): while a reply runs the draft goes the other way from Return.
    func followUpOther() {
        guard window.firstResponder === editor, cd_followup_submit(page, 1) != 0 else { return }
        wake()
    }
    /// A press on the tray, or Return, Command-Return or Esc in the composer (cd_action 25).
    func followUp() {
        let op = cd_followup_op(page), item = Int(cd_followup_item(page)), r = run
        switch op {
        case 20, 21:   // the draft steers the running reply, or goes last in the queue
            let text = editor.string
            guard !text.isEmpty else { return }
            if op == 20, steer(text) != nil { _ = takeDraft() }
            else if enqueue(text) != nil { _ = takeDraft() }
        case 22:       // the draft is the edited item's new text
            guard item >= 0, let q = queuedText(r, item) else { return }
            let text = takeDraft()
            if !text.isEmpty, queueWorker() { bindQueue(r); editQueued(q.id, text) }
            cd_followup_editing(page, -1)
            if !draftBeforeEdit.isEmpty { setDraft(draftBeforeEdit) }
            draftBeforeEdit = ""
        case 23:       // the edit is given up
            setDraft(draftBeforeEdit); draftBeforeEdit = ""
        case 24: interrupt(resubmit: false)
        case 25: UserDefaults.standard.set(cd_followup_queues(page) != 0, forKey: "followUpsQueueFirst")
        case 1, 2:     // Resume, Clear
            guard queueWorker() else { return }
            queueOp(op == 1 ? "resume" : "clear")
        case 3, 4, 6:  // Remove, Move up, Steer or Send now
            guard let q = queuedText(r, item), queueWorker() else { return }
            if op == 3 { queueOp("delete", q.id) }
            else if op == 4 { queueOp("move", q.id, to: item - 1) }
            else if cd_running(page) != 0 {
                // Steer: it leaves the queue and goes into the running reply
                queueOp("delete", q.id)
                if steer(q.text) == nil { _ = enqueue(q.text) }
            } else { queueOp("promote", q.id, now: true) }
        case 5:        // Edit: its text in the composer until Return saves it or Esc gives it up
            guard let q = queuedText(r, item) else { return }
            if cd_followup_edited(page) < 0 { draftBeforeEdit = editor.string }   // editing another, its draft stays the one kept
            cd_followup_editing(page, Int32(item))
            setDraft(q.text)
        case 7: interrupt(resubmit: true)
        case 9:        // a steer not taken goes last in the queue
            guard let st = steerText(r, item), queueWorker(), enqueue(st.text) != nil else { return }
            cd_steer_dismiss(page, Int32(item))
        default: break
        }
        wake()
    }
    /// The queue a chat kept (the worker's file), shown as it is when the chat opens with no worker
    /// on it: one that was waiting comes back paused, as the worker would load it (mwq_from).
    func showKeptQueue(_ id: String) {
        let env = ProcessInfo.processInfo.environment
        let home = env["CODER_HOME"].flatMap { $0.isEmpty ? nil : $0 } ?? env["HOME"].flatMap { $0.isEmpty ? nil : $0 } ?? FileManager.default.homeDirectoryForCurrentUser.path
        let url = URL(fileURLWithPath: home).appendingPathComponent(".openagents/coder/sessions/" + id + ".queue.json")
        guard let data = try? Data(contentsOf: url), let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let items = obj["items"] as? [[String: Any]], !items.isEmpty else { return }
        var paused = obj["paused"] as? Bool ?? false, reason = obj["reason"] as? String ?? ""
        let active = obj["active"] as? String ?? ""
        if !paused || !active.isEmpty { paused = true; reason = "restored" }
        var shown: [[String: String]] = []
        for one in items {
            guard let qid = one["id"] as? String, let text = one["text"] as? String else { continue }
            shown.append(["id": qid, "text": text]); run.queued[qid] = text
        }
        let line: [String: Any] = ["kind": "queue", "paused": paused, "reason": reason, "active": "", "items": shown]
        guard let out = try? JSONSerialization.data(withJSONObject: line), let text = String(data: out, encoding: .utf8) else { return }
        cd_line(page, text)
    }
}
