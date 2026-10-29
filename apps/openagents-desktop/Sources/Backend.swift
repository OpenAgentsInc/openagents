// The local stand-in for oa-api's /api/chat: chat.coax asks it what it asks
// openagents.com (GET /api/chat/state, POST /api/chat/send, model, stop),
// and each turn runs the user's own `claude` CLI on their own login
// (stream-json), so no key is stored here. The answers are chat.lines
// (coax/web/api/migrations/0021_chat.sql), the same lines the web draws.
import Foundation

struct Tool: Codable { var name: String; var detail: String }

final class Turn: Codable {
  var id: Int; var status: String; var at: Date; var model: String
  var text: String; var answer = ""; var error = ""; var working = ""; var cost = 0.0
  var tools: [Tool] = []
  init(id: Int, text: String, model: String) { self.id = id; self.text = text; self.model = model; status = "queued"; at = Date() }
}

final class Conversation: Codable {
  var id: Int; var title: String; var model: String; var updated: Date; var session: String?
  var turns: [Turn] = []
  init(id: Int, title: String, model: String) { self.id = id; self.title = title; self.model = model; updated = Date() }
  var busy: Bool { turns.contains { $0.status == "queued" || $0.status == "running" } }
}

struct Saved: Codable { var next: Int; var conversations: [Conversation]; var effort: String?; var lab: Conversation? }

final class Backend {
  private let lock = NSLock()
  private var next = 1
  private var effort = "high"
  // Zeron's streaming (crates/engine/src/sessions.rs): every delta marks the state dirty, and the
  // host pushes it at most every 120 ms (STREAM_COMMIT_MS) for the conversation last asked for.
  private var dirty = false
  private var lastShown = -1
  private var lastLab = false
  // The Lab: one thread of its own, apart from the chats (/api/lab/).
  private var lab = Conversation(id: 0, title: "Lab", model: "claude-opus-5-5")
  private var conversations: [Conversation] = []
  private var running: [Int: Process] = [:]
  private var partial: [Int: String] = [:]
  private let file: URL

  init() {
    let dir = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0].appendingPathComponent("OpenAgents")
    try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    file = dir.appendingPathComponent("chat.json")
    if let data = try? Data(contentsOf: file), let saved = try? JSONDecoder().decode(Saved.self, from: data) {
      next = saved.next; conversations = saved.conversations; effort = saved.effort ?? effort; lab = saved.lab ?? lab
      for c in conversations + [lab] { for t in c.turns where t.status == "queued" || t.status == "running" { t.status = "stopped"; t.error = "the app was closed" } }
    }
  }

  private func save() {
    if let data = try? JSONEncoder().encode(Saved(next: next, conversations: conversations, effort: effort, lab: lab)) { try? data.write(to: file, options: .atomic) }
  }

  // One request, answered "STATUS\nbody" as host.js answers chat.wasm.
  func handle(url: String, body: Data?, post: Bool) -> String {
    lock.lock(); defer { lock.unlock() }
    let parts = url.split(separator: "?", maxSplits: 1)
    let path = String(parts[0])
    var query: [String: String] = [:]
    if parts.count > 1 { for kv in parts[1].split(separator: "&") { let p = kv.split(separator: "=", maxSplits: 1); if p.count == 2 { query[String(p[0])] = String(p[1]) } } }
    let json = (body.flatMap { try? JSONSerialization.jsonObject(with: $0) } as? [String: Any]) ?? [:]
    switch (post, path) {
    case (false, "/api/lab/state"): lastLab = true; return "200\n" + labLines()
    case (true, "/api/lab/send"):
      let text = (json["text"] as? String ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
      if text.isEmpty { return "400\n{\"error\": \"nothing to send\"}" }
      if lab.busy { return "409\n{\"error\": \"a turn is still running\"}" }
      let t = Turn(id: next, text: text, model: "mammoth-1"); next += 1
      t.status = "running"; t.working = "routing"
      lab.turns.append(t); lab.updated = Date(); save()
      route(t)
      return "200\n{\"turn\": \(t.id)}"
    case (true, "/api/lab/stop"):
      if let t = lab.turns.last(where: { $0.status == "queued" || $0.status == "running" }) {
        t.status = "stopped"; t.error = "stopped"; t.working = ""
        running[t.id]?.terminate(); running[t.id] = nil; dirty = true; save()
      }
      return "200\n{}"
    case (false, "/api/chat/state"): lastLab = false; lastShown = Int(query["c"] ?? "") ?? -1; return "200\n" + lines(shown: lastShown)
    case (true, "/api/chat/send"): return send(json)
    case (true, "/api/chat/model"):
      if let c = find(json["c"]), let m = json["model"] as? String { c.model = m; save() }
      return "200\n{}"
    case (true, "/api/chat/effort"):
      if let e = json["effort"] as? String, ["low", "medium", "high", "xhigh", "max"].contains(e) { effort = e; save() }
      return "200\n{}"
    case (true, "/api/chat/stop"):
      if let c = find(json["c"]), let t = c.turns.last(where: { $0.status == "queued" || $0.status == "running" }) {
        t.status = "stopped"; t.error = "stopped"; t.working = ""
        running[t.id]?.terminate(); running[t.id] = nil; save()
      }
      return "200\n{}"
    case (true, "/api/chat/key"), (true, "/api/chat/key_remove"):
      return "400\n{\"error\": \"This app uses your own Claude Code login: run claude in a terminal to log in.\"}"
    default: return "404\n{\"error\": \"not found\"}"
    }
  }

  // The state as it now stands, once it has changed since the last push.
  func pushed() -> String? {
    lock.lock(); defer { lock.unlock() }
    if !dirty { return nil }
    dirty = false
    return lastLab ? labLines() : lines(shown: lastShown)
  }

  private func find(_ v: Any?) -> Conversation? {
    let id = (v as? String).flatMap { Int($0) } ?? (v as? Int) ?? -1
    return conversations.first { $0.id == id }
  }

  private func send(_ json: [String: Any]) -> String {
    let text = (json["text"] as? String ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
    if text.isEmpty { return "400\n{\"error\": \"nothing to send\"}" }
    let model = json["model"] as? String ?? "claude-opus-5-5"
    let c: Conversation
    if let found = find(json["c"]) {
      if found.busy { return "409\n{\"error\": \"a turn is still running\"}" }
      c = found; c.model = model
    } else {
      c = Conversation(id: next, title: String(text.prefix(60)), model: model); next += 1
      conversations.append(c)
    }
    let t = Turn(id: next, text: text, model: model); next += 1
    c.turns.append(t); c.updated = Date()
    save()
    start(c, t)
    return "200\n{\"c\": \(c.id), \"turn\": \(t.id), \"model\": \"\(model)\"}"
  }

  // The Lab: Mammoth decides where the message goes, and the decision is the answer (no harness runs yet).
  private func route(_ t: Turn) {
    Router.decide(t.text) { [weak self] json, ok in
      guard let self else { return }
      self.lock.lock(); defer { self.lock.unlock() }
      if t.status != "running" { return }
      t.answer = json; t.status = ok ? "done" : "failed"; t.working = ""
      if !ok { t.error = "the router didn't answer" }
      self.dirty = true; self.save()
    }
  }

  // ---- the claude CLI, on the user's own login

  static let claudePath: String? = {
    let home = NSHomeDirectory()
    for p in ["\(home)/.local/bin/claude", "/opt/homebrew/bin/claude", "/usr/local/bin/claude", "\(home)/.claude/local/claude"]
      where FileManager.default.isExecutableFile(atPath: p) { return p }
    let probe = Process(); let out = Pipe()
    probe.executableURL = URL(fileURLWithPath: "/bin/zsh"); probe.arguments = ["-lc", "command -v claude"]; probe.standardOutput = out
    try? probe.run(); probe.waitUntilExit()
    let found = String(data: out.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
    return found.isEmpty ? nil : found
  }()

  private func start(_ c: Conversation, _ t: Turn) {
    guard let claude = Backend.claudePath else {
      t.status = "failed"; t.error = "Claude Code isn't installed: curl -fsSL https://claude.ai/install.sh | bash"; return
    }
    let p = Process()
    p.executableURL = URL(fileURLWithPath: claude)
    var args = ["-p", t.text, "--output-format", "stream-json", "--verbose", "--include-partial-messages", "--model", t.model, "--effort", effort]
    if let s = c.session { args += ["--resume", s] }
    p.arguments = args
    p.currentDirectoryURL = URL(fileURLWithPath: NSHomeDirectory())
    var env = ProcessInfo.processInfo.environment
    env["ANTHROPIC_API_KEY"] = nil
    env["PATH"] = "\(NSHomeDirectory())/.local/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:" + (env["PATH"] ?? "")
    p.environment = env
    p.standardInput = FileHandle.nullDevice
    let out = Pipe(), err = Pipe()
    p.standardOutput = out; p.standardError = err
    var pending = Data()
    let turnId = t.id
    out.fileHandleForReading.readabilityHandler = { [weak self] h in
      let chunk = h.availableData
      if chunk.isEmpty { h.readabilityHandler = nil; return }
      pending.append(chunk)
      while let nl = pending.firstIndex(of: 10) {
        let line = pending[pending.startIndex..<nl]
        pending.removeSubrange(pending.startIndex...nl)
        self?.event(line, conv: c, turn: t)
      }
    }
    var stderr = Data()
    err.fileHandleForReading.readabilityHandler = { h in let d = h.availableData; if d.isEmpty { h.readabilityHandler = nil } else { stderr.append(d) } }
    p.terminationHandler = { [weak self] proc in
      guard let self else { return }
      self.lock.lock(); defer { self.lock.unlock() }
      self.running[turnId] = nil
      if t.status == "queued" || t.status == "running" {
        t.status = proc.terminationStatus == 0 ? "done" : "failed"
        if t.status == "failed" && t.error.isEmpty {
          let why = String(data: stderr, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
          t.error = why.isEmpty ? "claude exited with \(proc.terminationStatus)" : String(why.suffix(400))
        }
      }
      t.working = ""
      self.dirty = true
      self.save()
    }
    do { try p.run(); running[t.id] = p; t.status = "running"; t.working = "thinking" }
    catch { t.status = "failed"; t.error = "couldn't start claude: \(error.localizedDescription)" }
  }

  private func event(_ line: Data, conv c: Conversation, turn t: Turn) {
    guard let e = try? JSONSerialization.jsonObject(with: line) as? [String: Any] else { return }
    lock.lock(); defer { lock.unlock() }
    dirty = true
    if t.status != "running" { return }
    switch e["type"] as? String {
    case "system":
      if let s = e["session_id"] as? String { c.session = s }
    case "stream_event":
      guard let ev = e["event"] as? [String: Any] else { return }
      if ev["type"] as? String == "content_block_delta", let d = ev["delta"] as? [String: Any], d["type"] as? String == "text_delta", let s = d["text"] as? String {
        partial[t.id, default: ""] += s; t.working = "writing"
      } else if ev["type"] as? String == "content_block_start", let b = ev["content_block"] as? [String: Any], b["type"] as? String == "tool_use" {
        t.working = "using \(b["name"] as? String ?? "a tool")"
      }
    case "assistant":
      guard let m = e["message"] as? [String: Any], let content = m["content"] as? [[String: Any]] else { return }
      for b in content {
        if b["type"] as? String == "text", let s = b["text"] as? String {
          t.answer += (t.answer.isEmpty ? "" : "\n\n") + s; partial[t.id] = nil
        } else if b["type"] as? String == "tool_use" {
          let name = b["name"] as? String ?? "tool"
          let input = b["input"] as? [String: Any] ?? [:]
          let detail = ["file_path", "command", "pattern", "url", "query", "description", "path"].lazy.compactMap { input[$0] as? String }.first ?? ""
          t.tools.append(Tool(name: name, detail: detail)); t.working = "using \(name)"
        }
      }
    case "result":
      if let s = e["session_id"] as? String { c.session = s }
      t.cost = e["total_cost_usd"] as? Double ?? 0
      if t.answer.isEmpty, let r = e["result"] as? String { t.answer = r }
      partial[t.id] = nil
      if e["is_error"] as? Bool == true { t.status = "failed"; t.error = (e["result"] as? String) ?? "the turn failed" } else { t.status = "done" }
      t.working = ""
      c.updated = Date()
    default: break
    }
  }

  // ---- chat.lines

  private static let stamp: DateFormatter = { let f = DateFormatter(); f.dateFormat = "MMM d HH:mm"; return f }()
  private func cell(_ s: String) -> String { s.replacingOccurrences(of: "|", with: "/").replacingOccurrences(of: "\n", with: " ") }
  private func escaped(_ s: String) -> String {
    s.replacingOccurrences(of: "\\", with: "\\\\").replacingOccurrences(of: "\n", with: "\\n").replacingOccurrences(of: "|", with: "\\p")
  }

  private func lines(shown asked: Int) -> String {
    let list = conversations.sorted { $0.updated > $1.updated }
    let shown = asked > 0 ? conversations.first { $0.id == asked } : (asked < 0 ? list.first : nil)
    let busy = conversations.contains { $0.busy }
    if asked < 0, let c = shown { lastShown = c.id }
    var out = "Z|0|0|local||0|\(busy ? 1 : 0)|\(shown?.id ?? 0)|\(effort)\n"
    for c in list { out += "C|\(c.id)|\(cell(c.title))|\(c.model)|\(Backend.stamp.string(from: c.updated))|\(c.busy ? 1 : 0)\n" }
    if let c = shown {
      out += "S|\(c.id)|\(cell(c.title))|\(c.model)|\(c.session == nil ? 0 : 1)\n"
      out += turnLines(c)
    }
    return out
  }

  private func labLines() -> String {
    "Z|0|0|local||0|\(lab.busy ? 1 : 0)|lab|\(effort)\n" + turnLines(lab)
  }

  private func turnLines(_ c: Conversation) -> String {
    var out = ""
    for t in c.turns {
        let answer = t.answer + (partial[t.id].map { (t.answer.isEmpty ? "" : "\n\n") + $0 } ?? "")
        out += "U|\(t.id)|\(t.status)|\(Backend.stamp.string(from: t.at))|\(t.model)|local|\(escaped(t.text))|\(escaped(answer))|\(cell(t.error))|\(cell(t.working))|\(String(format: "%.4f", t.cost))\n"
        for tool in t.tools { out += "T|\(t.id)|\(cell(tool.name))|\(cell(tool.detail))\n" }
    }
    return out
  }
}
