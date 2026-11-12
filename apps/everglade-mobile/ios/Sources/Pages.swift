// Firm reason: 6 (UIKit and Metal have no C API; this host holds no product logic).
// The web's realms in the iPhone app (everglade/docs/app-scenes-spec.md, section 4): Everglade
// itself, the Archery Range and Experimental run natively through Aethyr's page host
// (aethyr/host/apple/page-host.h), the one the Mac app opens them with, from the same table of
// rows (apps/everglade-desktop/Sources/Realms.swift). This file opens a row's page from the app's
// own files, steps it, and hands it fingers as a browser hands a page touches: each with its own
// number, so the realm's own touch controls (the thumb's stick, a slot held to draw the bow, a drag
// that looks) work as they do on the web. Every call into a realm runs on the deep thread
// (main.swift Deep): the web's character code passes megabytes by value.
import UIKit
import Metal

@MainActor final class RealmPages {
  let state: UnsafeMutableRawPointer
  let deep: Deep
  let device: MTLDevice, queue: MTLCommandQueue
  // The realms, each once its row is first opened, by its world's number.
  private(set) var pages: [Int32: AEPageHost] = [:]
  private var sounds: [Int32: AEPageAudio] = [:]   // each open realm's sound (aethyr/host/apple/PageAudio.swift)
  private var tried: Set<Int32> = []
  // The realm being opened, off the frame (#13152): its world, and where the deep thread leaves the
  // page. Until it ends nothing else is asked of the deep thread, so the screen never waits on it,
  // and the scene draws the world's name and a line (face.h eg_opening).
  private final class Opened { var page: AEPageHost?; var why: NSString? }
  private var begun: (world: Int32, into: Opened)?
  var opening: Bool { begun != nil }
  private var shown: Int32 = 0          // the world whose realm is on screen; 0 when none is
  // The fingers that are a realm's: each one's number, and the page it went down on.
  private var fingers: [ObjectIdentifier: (id: Int32, page: AEPageHost)] = [:]
  private var nextFinger: Int32 = 1
  // The realm's box in the view's points: the screen inside the notch, the corners and the home bar.
  var box = CGRect.zero
  // "--offline": no socket and no request leaves the phone (a shot).
  let offline = CommandLine.arguments.contains("--offline")
  // The phone's motion sensor for the realms' gyro (aethyr/cam/gyro.coax): 1 there, 3 none. main.swift sets it.
  var motion: Int32 = 3

  init(state: UnsafeMutableRawPointer, deep: Deep, device: MTLDevice, queue: MTLCommandQueue) {
    self.state = state; self.deep = deep; self.device = device; self.queue = queue
    // A realm's files are in the app: its row opens a world, not a card.
    for world in Realms.table.keys where RealmPages.files(world) != nil { eg_runs(state, world, 1) }
  }

  // The realm of the world that is open, when it has one (none while another is opening: its thread is busy).
  var page: AEPageHost? { opening ? nil : pages[eg_scene(state)] }

  // A realm's library in the app. iOS loads code only from a framework.
  static func library(_ r: Realm) -> URL? {
    Bundle.main.privateFrameworksURL?.appendingPathComponent("\(r.module).framework/\(r.module)")
  }
  // A realm's folder in the app (its shaders and bindings), when the app was built with it.
  static func files(_ world: Int32) -> URL? {
    guard let r = Realms.table[world], let res = Bundle.main.resourceURL, let lib = library(r),
          FileManager.default.fileExists(atPath: lib.path),
          FileManager.default.fileExists(atPath: res.appendingPathComponent(r.pack).path) else { return nil }
    return res.appendingPathComponent(r.module)
  }

  // Opens a world's realm: its library, shaders, pack, fonts and pictures from the app. What a
  // browser would keep in its storage (your character, your place, your settings) is kept in the
  // app's own folder, one for every realm, as one site's pages share a browser's storage.
  private func open(_ world: Int32, points: CGSize) {
    tried.insert(world)
    // A module runs one page at a time (Realms.sharing): opening a world closes any other page of its module.
    for other in Realms.sharing(world) where pages[other] != nil { close(other) }
    guard let r = Realms.table[world], let dir = RealmPages.files(world), let res = Bundle.main.resourceURL, let lib = RealmPages.library(r),
          let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first else { eg_runs(state, world, 0); return }
    let c = AEPageConfig()
    c.producer = lib.path; c.shaders = dir.path
    c.pack = res.appendingPathComponent(r.pack).path
    c.fonts = res.appendingPathComponent("fonts").path; c.reading = "Spectral-Medium"; c.title = "Cinzel"; c.titleWeight = 600
    c.web = res.appendingPathComponent("web").path
    // A shot keeps nothing of yours: its storage is a folder of its own, emptied as it opens.
    let store = support.appendingPathComponent(offline ? "realm-shot" : "realm")
    if offline && pages.isEmpty { try? FileManager.default.removeItem(at: store) }
    c.store = store.path
    c.origin = "https://openagents.com"; c.path = r.path
    c.realm = r.realm; c.slot = r.slot; c.style = "painterly"; c.clock = "fixed:0.5"
    c.offline = offline
    // These builds go to the team (TestFlight): their sessions are marked as a driven browser's
    // are, as the Mac app's are, and never count as visitors.
    c.automated = true
    // A touch screen: the realm draws its touch controls, and the pack's layers take a phone's
    // tier, which is Coax's to say (aethyr/render/texture-format.coax).
    c.touch = true
    // The screen in points, as a browser's screen.width says it: the governor reads its short side to
    // tell a phone from a tablet (aethyr/core/quality.coax), and a phone in pixels read as a tablet (#13190).
    c.screen = UIScreen.main.bounds.size
    c.layerSide = { declared in Int32(tf_side(Int64(declared), true, tf_pick(0, 0, true, 0))) }
    // The page opens on the deep thread while the frames go on: reading a pack and drawing its
    // pictures takes a few seconds on a phone, and the screen would hold still for all of them.
    let into = Opened()
    begun = (world, into)
    eg_opening(state, 1)
    deep.begin(RealmPages.opener(c, device: device, queue: queue, into: into))
  }
  // What the deep thread runs to open a page. Made outside the main actor: it runs on that thread.
  private nonisolated static func opener(_ c: AEPageConfig, device: MTLDevice, queue: MTLCommandQueue, into: Opened) -> () -> Void {
    return { var why: NSString?; into.page = AEPageHost.open(c, device: device, queue: queue, why: &why); into.why = why }
  }
  // The page begun has opened, or did not.
  private func opened(_ world: Int32, _ into: Opened) {
    begun = nil
    eg_opening(state, 0)
    pages[world] = into.page
    if let page = into.page { let m = motion; deep.run { page.motionStatus(m) } }
    // A shot never plays: its sound is rendered offline, in the shot's time.
    if let page = into.page, let res = Bundle.main.resourceURL { sounds[world] = AEPageAudio(page: page, resources: res, offline: offline) }
    if into.page == nil {
      NSLog("Everglade: the realm did not open: %@. Rebuild with apps/everglade-mobile/build.sh.", into.why ?? "unknown")
      eg_runs(state, world, 0)
    }
  }
  private func close(_ world: Int32) {
    guard let p = pages[world] else { return }
    if shown == world { shown = 0 }
    fingers = fingers.filter { $0.value.page !== p }
    sounds[world]?.close(); sounds[world] = nil
    deep.run { p.close() }
    pages[world] = nil; tried.remove(world)
  }
  func closeAll() { for w in Array(pages.keys) { close(w) } }

  // One frame. A realm's row, first opened: the realm starts. Leaving it lets its keys and fingers
  // go; it keeps its state. Returns the realm on screen, drawn, or nil when the open world has none.
  func frame(scale: CGFloat, dt: Float) -> AEPageHost? {
    let world = eg_scene(state)
    if let b = begun {
      guard deep.ended() else { return nil }   // still opening: the scene draws its line
      opened(b.world, b.into)
    }
    let on: Int32 = pages[world] != nil ? world : 0
    if on != shown {
      let was = pages[shown], now = pages[on]
      if let was { letGo(was) }
      deep.run { was?.shown(false); now?.shown(true) }
      sounds[shown]?.show(false); sounds[on]?.show(true)
      shown = on
    }
    // A realm's row, first opened: the page before it has been let go above; this one begins to open.
    if Realms.table[world] != nil && pages[world] == nil && !tried.contains(world) { open(world, points: box.size); if opening { return nil } }
    let realm = pages[on]
    deep.run {
      for (w, p) in pages where w != on { p.idle() }   // out of sight a realm draws nothing, and still hears its room
      if let realm { _ = realm.frame(box.size, scale: scale, dt: dt, wait: false) }
    }
    for (w, s) in sounds { if w == on || pages[w] != nil { s.step(dt) } }
    return realm
  }

  // A finger for the realm on screen: phase 0 down, 1 moved, 2 up, 3 cancelled; `at` in the view's
  // points. `key` is whatever names the finger from down to up (a UITouch).
  func finger(_ key: AnyObject, at p: CGPoint, phase: Int32) {
    let k = ObjectIdentifier(key)
    if phase == 0 {
      guard let page else { return }
      fingers[k] = (nextFinger, page); nextFinger = nextFinger == Int32.max ? 1 : nextFinger + 1
      sounds[shown]?.gesture()   // a touch is the gesture the realm's sound starts on, before the realm hears it
    }
    guard let f = fingers[k] else { return }
    if phase >= 2 { fingers[k] = nil }
    let at = CGPoint(x: p.x - box.minX, y: p.y - box.minY)
    deep.run { f.page.touch(at, phase: phase, finger: f.id) }
  }
  func owns(_ key: AnyObject) -> Bool { fingers[ObjectIdentifier(key)] != nil }
  // Every finger on a page is let go where it is: the page is going out of sight.
  private func letGo(_ p: AEPageHost) {
    let gone = fingers.filter { $0.value.page === p }
    for (k, f) in gone { fingers[k] = nil; deep.run { p.touch(.zero, phase: 3, finger: f.id) } }
  }
  // The app lost the screen, or has it back.
  func active(_ on: Bool) {
    guard let p = pages[shown] else { return }
    if !on { letGo(p) }
    deep.run { p.focus(on) }
    sounds[shown]?.show(on)
  }
  // The realm's text field (a character's name): what the phone's keyboard typed, Return and Delete
  // in the host contract's codes (aethyr/host/README.md: Enter -2, Backspace -3).
  var wantsText: Bool { var w = false; if !opening, let p = pages[shown] { deep.run { w = p.wantsText() } }; return w }
  func typed(_ text: String) {
    guard !opening, let p = pages[shown] else { return }
    deep.run { if text == "\n" { _ = p.key(-2, mods: 0, down: true); _ = p.key(-2, mods: 0, down: false) } else { p.text(text) } }
  }
  func deleted() {
    guard !opening, let p = pages[shown] else { return }
    deep.run { _ = p.key(-3, mods: 0, down: true); _ = p.key(-3, mods: 0, down: false) }
  }
  // A key held or let go, in the host contract's codes, for a shot's script.
  func key(_ code: Int32, down: Bool) {
    guard !opening, let p = pages[shown] else { return }
    deep.run { _ = p.key(code, mods: 0, down: down) }
  }
  // The phone's turn for the realm on screen (aethyr/cam/gyro.coax): whether it wants it now, and the turn.
  var wantsMotion: Bool { var w = false; if !opening, let p = pages[shown] { deep.run { w = p.wantsMotion() } }; return w }
  func tilt(_ yaw: Float, _ pitch: Float) {
    guard !opening, let p = pages[shown] else { return }
    deep.run { p.tilt(yaw, pitch: pitch) }
  }
  // What the realm on screen has played, for a shot's script (a shot renders its sound offline).
  var heard: String { sounds[shown]?.summary() ?? "no sound" }
  // The realm's agent driver (aethyr/agent/drive.coax), for a shot's script.
  func drive(_ command: String) -> String {
    var said: String?
    if !opening, let p = pages[shown] { deep.run { said = p.drive(command) } }
    return said ?? "this realm has no driver"
  }
}

// A shot's script, "--do STEPS": what the Mac app's --do is for a window, for fingers. The
// simulator has no way to touch the screen from outside, so the app plays the fingers itself, in
// the view's points, through the same path a real touch takes. Steps are split by `;`:
//   wait:S                 S seconds
//   down:F:X,Y  move:F:X,Y  up:F:X,Y    finger F goes down, moves, lifts
//   tap:X,Y                a finger down and up
//   drag:F:X,Y:S           finger F, already down, moves to X,Y over S seconds
//   key:CODE:S             holds a key (the host contract's code: 119 is W) for S seconds
//   tilt:YAW,PITCH:S       the phone turns that far (radians, the page host's tilt) over S seconds, as
//                          CoreMotion would say it: the simulator has no gyro
//   drive:COMMAND          the realm's agent driver; its answer goes to the log
//   sound                  what the realm has played so far goes to the log
//   mark:NAME              writes Documents/NAME, so whoever takes the picture knows the step was reached
@MainActor final class Script {
  private var steps: [[String]] = []
  private var wait = 0.0
  private var at: [String: CGPoint] = [:]
  private final class Key {}
  private var keys: [String: Key] = [:]
  init?(arguments: [String]) {
    guard let w = arguments.firstIndex(of: "--do"), w + 1 < arguments.count else { return nil }
    for part in arguments[w + 1].split(separator: ";") {
      let p = part.split(separator: ":", omittingEmptySubsequences: false).map { $0.trimmingCharacters(in: .whitespaces) }
      guard let verb = p.first else { continue }
      if verb == "tap", p.count > 1 { steps += [["down", "tap", p[1]], ["wait", "0.08"], ["up", "tap", p[1]], ["wait", "0.1"]] }
      else if verb == "drag", p.count > 3, let s = Double(p[3]) {
        let n = max(2, Int(s * 30))
        for i in 0..<n { steps += [["glide", p[1], p[2], String(n - i)], ["wait", String(s / Double(n))]] }
      } else if verb == "key", p.count > 2 { steps += [["keydown", p[1]], ["wait", p[2]], ["keyup", p[1]]] }
      else if verb == "tilt", p.count > 2, let s = Double(p[2]) {
        let n = max(2, Int(s * 60))
        for _ in 0..<n { steps += [["turn", p[1], String(n)], ["wait", String(s / Double(n))]] }
      }
      else { steps.append(p) }
    }
  }
  private func point(_ text: String) -> CGPoint? {
    let xy = text.split(separator: ",").compactMap { Double($0) }
    return xy.count == 2 ? CGPoint(x: xy[0], y: xy[1]) : nil
  }
  // Runs the steps that are due. `touch` is the view's own path for a finger.
  func step(_ dt: Float, pages: RealmPages, touch: (AnyObject, CGPoint, Int32) -> Void) {
    wait -= Double(dt)
    while wait <= 0, !steps.isEmpty {
      let p = steps.removeFirst(), verb = p[0]
      if verb == "wait", p.count > 1 { wait = Double(p[1]) ?? 0 }
      else if ["down", "move", "up"].contains(verb), p.count > 2, let xy = point(p[2]) {
        let key = keys[p[1]] ?? Key(); keys[p[1]] = key; at[p[1]] = xy
        touch(key, xy, verb == "down" ? 0 : verb == "move" ? 1 : 2)
        if verb == "up" { keys[p[1]] = nil }
      } else if verb == "glide", p.count > 3, let to = point(p[2]), let left = Double(p[3]), left >= 1, let key = keys[p[1]], let from = at[p[1]] {
        // one of the `left` equal moves still to make: the last lands on the point
        let xy = CGPoint(x: from.x + (to.x - from.x) / left, y: from.y + (to.y - from.y) / left)
        at[p[1]] = xy; touch(key, xy, 1)
      } else if verb == "keydown" || verb == "keyup", p.count > 1, let code = Int32(p[1]) { pages.key(code, down: verb == "keydown") }
      else if verb == "turn", p.count > 2, let yp = point(p[1]), let n = Double(p[2]), n >= 1 { pages.tilt(Float(yp.x / n), Float(yp.y / n)) }
      else if verb == "drive", p.count > 1 { NSLog("Everglade drive: %@", pages.drive(p[1...].joined(separator: ":"))) }
      else if verb == "sound" { NSLog("Everglade sound: %@", pages.heard) }
      else if verb == "mark", p.count > 1, let docs = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first {
        try? FileManager.default.createDirectory(at: docs, withIntermediateDirectories: true)
        try? Data().write(to: docs.appendingPathComponent(p[1]))
      }
    }
  }
}
