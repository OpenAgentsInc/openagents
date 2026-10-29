// The Lab's routing: this Mac's environment and the user's message go to
// Mammoth (router.coax), and its decision is written into the transcript as
// JSON. No harness runs yet.
import Foundation

enum Environment {
  // What the router is told about this Mac, gathered once per launch (the CLIs' versions take a moment).
  static let facts: [String: Any] = gather()

  private static func sysctl(_ name: String) -> String {
    var size = 0
    guard sysctlbyname(name, nil, &size, nil, 0) == 0, size > 0 else { return "" }
    var buf = [CChar](repeating: 0, count: size)
    guard sysctlbyname(name, &buf, &size, nil, 0) == 0 else { return "" }
    return String(cString: buf)
  }

  private static func run(_ path: String, _ args: [String]) -> String {
    let p = Process(), out = Pipe()
    p.executableURL = URL(fileURLWithPath: path); p.arguments = args
    p.standardOutput = out; p.standardError = out; p.standardInput = FileHandle.nullDevice
    var env = ProcessInfo.processInfo.environment
    env["PATH"] = "\(NSHomeDirectory())/.local/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:" + (env["PATH"] ?? "")
    for k in ["BASETEN_API_KEY", "ANTHROPIC_API_KEY", "OPENAI_API_KEY"] { env[k] = nil }
    p.environment = env
    guard (try? p.run()) != nil else { return "" }
    p.waitUntilExit()
    return String(data: out.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
  }

  static func which(_ name: String) -> String? {
    let home = NSHomeDirectory()
    for dir in ["\(home)/.local/bin", "/opt/homebrew/bin", "/usr/local/bin", "\(home)/.claude/local"] where FileManager.default.isExecutableFile(atPath: "\(dir)/\(name)") {
      return "\(dir)/\(name)"
    }
    let found = run("/bin/zsh", ["-lc", "command -v \(name)"])
    return found.hasPrefix("/") ? found : nil
  }

  private static func agent(_ name: String, versionArgs: [String]) -> [String: Any] {
    guard let path = which(name) else { return ["installed": false] }
    var a: [String: Any] = ["installed": true, "version": run(path, versionArgs).components(separatedBy: "\n").first ?? ""]
    if name == "codex" {
      a["logged_in"] = run(path, ["login", "status"]).lowercased().contains("logged in")
      if let config = try? String(contentsOfFile: "\(NSHomeDirectory())/.codex/config.toml", encoding: .utf8),
         let line = config.components(separatedBy: "\n").first(where: { $0.hasPrefix("model =") }) {
        a["default_model"] = line.components(separatedBy: "\"").dropFirst().first ?? ""
      }
    }
    if name == "claude" {
      a["logged_in"] = FileManager.default.fileExists(atPath: "\(NSHomeDirectory())/.claude.json")
    }
    return a
  }

  private static func gather() -> [String: Any] {
    let info = ProcessInfo.processInfo
    let bundle = Bundle.main.infoDictionary ?? [:]
    let disk = (try? FileManager.default.attributesOfFileSystem(forPath: NSHomeDirectory()))?[.systemFreeSize] as? NSNumber
    return [
      "app": ["name": "OpenAgents for Mac", "bundle_id": Bundle.main.bundleIdentifier ?? "", "version": bundle["CFBundleShortVersionString"] ?? "", "build": bundle["CFBundleVersion"] ?? ""],
      "os": ["name": "macOS", "version": info.operatingSystemVersionString, "arch": sysctl("hw.machine")],
      "hardware": ["model": sysctl("hw.model"), "chip": sysctl("machdep.cpu.brand_string"), "cores": info.activeProcessorCount,
                   "memory_gb": Int(info.physicalMemory / 1_073_741_824), "disk_free_gb": (disk?.int64Value ?? 0) / 1_073_741_824],
      "locale": Locale.current.identifier, "time_zone": TimeZone.current.identifier,
      "working_directory": NSHomeDirectory(),
      "agents": ["claude_code": agent("claude", versionArgs: ["--version"]), "codex": agent("codex", versionArgs: ["--version"])],
    ]
  }
}

enum Router {
  // Dev only, on this Mac: Mammoth's key from ~/work/.secrets/baseten.env when it isn't already in the
  // environment. curl reads it from there (coax/lib/systemone/transport.coax); it is never shown.
  static func loadKey() {
    if let k = ProcessInfo.processInfo.environment["BASETEN_API_KEY"], !k.isEmpty { return }
    guard let text = try? String(contentsOfFile: "\(NSHomeDirectory())/work/.secrets/baseten.env", encoding: .utf8) else { return }
    for line in text.components(separatedBy: "\n") where line.hasPrefix("BASETEN_API_KEY=") {
      var v = String(line.dropFirst("BASETEN_API_KEY=".count)).trimmingCharacters(in: .whitespaces)
      if v.count >= 2, v.first == "\"", v.last == "\"" { v = String(v.dropFirst().dropLast()) }
      setenv("BASETEN_API_KEY", v, 1)
    }
  }

  private static let queue = DispatchQueue(label: "router")

  // Mammoth's decision for text, as the JSON the transcript shows, or the reason it failed.
  static func decide(_ text: String, done: @escaping (_ json: String, _ ok: Bool) -> Void) {
    queue.async {
      let env = Environment.facts
      let state = try! JSONSerialization.data(withJSONObject: ["message": text, "environment": env], options: [.sortedKeys])
      let started = Date()
      var out = [UInt8](repeating: 0, count: 65536)
      let n = state.withUnsafeBytes { s in dk_route(s.bindMemory(to: UInt8.self).baseAddress, Int32(state.count), &out, Int32(out.count)) }
      let ms = Int(Date().timeIntervalSince(started) * 1000)
      if n < 0 {
        let why = String(decoding: out[0..<Int(-n)], as: UTF8.self)
        done(pretty(Ordered([("router", "mammoth-1"), ("error", why), ("latency_ms", ms)])), false); return
      }
      guard let response = try? JSONSerialization.jsonObject(with: Data(out[0..<Int(n)])) as? [String: Any],
            let answers = response["answers"] as? [String: Any] else { done(pretty(Ordered([("router", "mammoth-1"), ("error", "unreadable answer")])), false); return }
      done(pretty(summary(answers, env: env, ms: ms, usage: response["usage"])), true)
    }
  }

  private static func choice(_ a: [String: Any], _ name: String) -> (String, Double, [String: Any]) {
    let q = a[name] as? [String: Any] ?? [:]
    let key = q["choice"] as? String ?? ""
    let probs = q["probabilities"] as? [String: Any] ?? [:]
    return (key, (probs[key] as? NSNumber)?.doubleValue ?? 0, probs)
  }
  private static func round(_ x: Double) -> Double { (x * 1000).rounded() / 1000 }
  // A distribution, most likely first.
  private static func ranked(_ probs: [String: Any]) -> Ordered {
    Ordered(probs.map { ($0.key, round(($0.value as? NSNumber)?.doubleValue ?? 0) as Any) }.sorted { ($0.1 as! Double) > ($1.1 as! Double) })
  }

  // The decision: where it goes, how sure Mammoth is, and what to run it with.
  static func summary(_ a: [String: Any], env: [String: Any], ms: Int, usage: Any?) -> Ordered {
    let (harness, certainty, hp) = choice(a, "harness")
    let (effort, effortP, ep) = choice(a, "effort")
    let (claude, claudeP, cp) = choice(a, "claude_model")
    let difficulty = ((a["difficulty"] as? [String: Any])?["score"] as? NSNumber)?.doubleValue ?? 0
    let levels = ["trivial", "easy", "moderate", "hard", "very hard"]
    let local = ((a["local_work"] as? [String: Any])?["noul"] as? NSNumber)?.doubleValue ?? 0
    let agents = env["agents"] as? [String: Any] ?? [:]
    let codexModel = (agents["codex"] as? [String: Any])?["default_model"] as? String ?? "gpt-6.1-sol"
    var run: [String: Any] = ["effort": effort]
    if harness == "claude_code" { run["model"] = claude } else if harness == "codex" {
      run["model"] = codexModel; run["effort"] = effort == "max" ? "xhigh" : effort
    }
    var d: [(String, Any)] = [
      ("route", harness), ("certainty", round(certainty)),
      ("run", Ordered(run.sorted { $0.key > $1.key }.map { ($0.key, $0.value) })),
      ("difficulty", Ordered([("level", levels[max(0, min(4, Int(difficulty.rounded())))]), ("score", round(difficulty)), ("of", 4)])),
      ("local_work", round(local)),
      ("probabilities", Ordered([("route", ranked(hp)), ("effort", ranked(ep)), ("claude_model", ranked(cp))])),
      ("chosen_with", Ordered([("effort", round(effortP)), ("claude_model", round(claudeP))])),
      ("router", "mammoth-1 (baseten-mammoth-27b)"), ("latency_ms", ms), ("harness_started", false),
    ]
    if let u = usage as? [String: Any] { d.append(("usage", Ordered(u.sorted { $0.key < $1.key }.map { ($0.key, $0.value) }))) }
    return Ordered(d)
  }

  // Pretty JSON that keeps the order it was given and writes numbers to three places.
  static func pretty(_ o: Any) -> String { "```json\n" + write(o, indent: "") + "\n```" }
  private static func write(_ v: Any, indent: String) -> String {
    let inner = indent + "  "
    switch v {
    case let o as Ordered:
      if o.pairs.isEmpty { return "{}" }
      return "{\n" + o.pairs.map { inner + quote($0.0) + ": " + write($0.1, indent: inner) }.joined(separator: ",\n") + "\n" + indent + "}"
    case let d as [String: Any]: return write(Ordered(d.sorted { $0.key < $1.key }.map { ($0.key, $0.value) }), indent: indent)
    case let b as Bool: return b ? "true" : "false"
    case let i as Int: return String(i)
    case let x as Double:
      var t = String(format: "%.3f", x)
      while t.hasSuffix("0") { t.removeLast() }
      if t.hasSuffix(".") { t.removeLast() }
      return t
    case let n as NSNumber: return CFGetTypeID(n) == CFBooleanGetTypeID() ? (n.boolValue ? "true" : "false") : write(n.doubleValue, indent: indent)
    case let s as String: return quote(s)
    default: return "null"
    }
  }
  private static func quote(_ s: String) -> String {
    var out = "\""
    for c in s.unicodeScalars {
      switch c {
      case "\"": out += "\\\""
      case "\\": out += "\\\\"
      case "\n": out += "\\n"
      default: if c.value < 32 { out += String(format: "\\u%04x", c.value) } else { out.unicodeScalars.append(c) }
      }
    }
    return out + "\""
  }
}

struct Ordered { var pairs: [(String, Any)]; init(_ p: [(String, Any)]) { pairs = p } }
