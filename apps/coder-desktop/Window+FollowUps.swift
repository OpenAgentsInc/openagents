// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
// Follow-ups (GitHub #13715, #13716): steer a running reply, queue messages for after it, interrupt it.
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
}
