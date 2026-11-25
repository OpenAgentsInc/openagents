// Firm reason: 6 (AppKit/Metal/AVAudioEngine host wrapper over relocatable gamebuilder-core.o; all game/edit rules live in Coax).
// Embeds gamebuilder/core/ (Build, Play, Play-Test, .gbm meshes, SPLICEBOTS Farm, world.coax, CoreAudio PCM, --attach)
// inside the expandable right-side Metaverse Viewer & GameBuilder pane (GitHub #14043, #14044).
import AppKit
import AVFoundation
import Metal
import simd

#if OA_FULL

/// Synthesizes GameBuilder's Coax PCM sound bank (ed_play_sound) once and plays events on macOS CoreAudio
/// strictly when Play mode is active and unmuted.
final class GameBuilderAudioPlayer {
    static let shared = GameBuilderAudioPlayer()
    private var engine: AVAudioEngine?
    private var players: [AVAudioPlayerNode] = []
    private var nextPlayer = 0
    private var buffers: [Int32: AVAudioPCMBuffer] = [:]
    private var started = false

    func prepareIfNeeded() {
        guard !started else { return }
        let rate = Double(max(8000, oa_gb_get_sound_rate()))
        let count = oa_gb_get_sound_count()
        guard count > 0, let fmt = AVAudioFormat(standardFormatWithSampleRate: rate, channels: 1) else { return }
        let eng = AVAudioEngine()
        var nodes: [AVAudioPlayerNode] = []
        for _ in 0..<8 {
            let p = AVAudioPlayerNode()
            eng.attach(p)
            eng.connect(p, to: eng.mainMixerNode, format: fmt)
            nodes.append(p)
        }
        var scratch = [Float](repeating: 0, count: Int(rate * 1.5))
        for id in Int32(1)..<Int32(count) {
            let n = Int(oa_gb_get_sound_pcm(id, &scratch, Int32(scratch.count)))
            guard n > 0, let buf = AVAudioPCMBuffer(pcmFormat: fmt, frameCapacity: AVAudioFrameCount(n)) else { continue }
            buf.frameLength = AVAudioFrameCount(n)
            if let ch = buf.floatChannelData?[0] {
                for i in 0..<n { ch[i] = scratch[i] }
                buffers[id] = buf
            }
        }
        do {
            try eng.start()
            for p in nodes { p.play() }
            self.engine = eng
            self.players = nodes
            self.started = true
        } catch {
            // Audio device unavailable (e.g. headless CI); degrade silently without blocking Play.
        }
    }

    func playEvents(from handle: UnsafeMutableRawPointer, muted: Bool) {
        var evs = [Int32](repeating: 0, count: 24)
        let n = Int(oa_gb_poll_sound_events(handle, &evs, Int32(evs.count)))
        guard !muted, n > 0 else { return }
        prepareIfNeeded()
        guard started, !players.isEmpty else { return }
        for i in 0..<n {
            guard let buf = buffers[evs[i]] else { continue }
            let node = players[nextPlayer % players.count]
            nextPlayer &+= 1
            node.scheduleBuffer(buf, at: nil, options: [], completionHandler: nil)
        }
    }
}

struct GameBuilderPlaytestPin {
    let label: String
    let xCm: Int32
    let zCm: Int32
}

struct GameBuilderUiSnapshot {
    let tool: Int32
    let canUndo: Bool
    let canRedo: Bool
    let templateId: Int32
    let playing: Bool
    let levelNum: Int32
    let savedRev: Int32
    let saveDue: Bool
    let readOnly: Bool
    let changeCount: Int32
    let selCount: Int32
    let selId: Int32
    let selKind: Int32
    let fields: [Int32]
    let actorCount: Int32
    let claimsCount: Int32
    let selLabel: String
    let statusNote: String
    let camCoordsCm: [Int64] // [0..2]=sel/hit, [3..5]=eye, [6..8]=pivot
}

/// Owns one live GameBuilder Editor/Play/Farm/World session backed by gamebuilder-core.o.
final class GameBuilderSession {
    let key: String
    var gameId: String
    var levelId: String
    var levelNum: Int32
    var handle: UnsafeMutableRawPointer?
    var muted: Bool = true
    var matchMode: Int32 = 0
    var fighterClass: Int32 = 0
    var teamSize: Int32 = 3

    // Hydrated GPU mesh buffer for scrappers.gbm (MESH_VERTEX_WORDS = 7 floats / 28 bytes per vertex)
    var gbmVertexBuffer: MTLBuffer?
    var gbmMeshRanges: [Int32: (first: Int, count: Int)] = [:]
    var gbmLoaded: Bool = false
    var farmLoaded: Bool = false
    var needsHydration: Bool = false

    // Latest deterministic play-test results & jump-to-issue pins
    var playtestPins: [GameBuilderPlaytestPin] = []
    var lastPlaytestSummary: String = ""
    var lastPlaytestJson: String = ""

    // Live --attach Unix socket state (#14044)
    var attachListening: Bool = false
    var attachSocketPath: String = ""
    var attachRequestsServed: Int32 = 0

    static let baseDir: URL = {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".openagents/gamebuilder", isDirectory: true)
    }()
    static let levelsDir: URL = {
        baseDir.appendingPathComponent("Levels", isDirectory: true)
    }()

    init(key: String, gameId: String, levelId: String, viewportSize: CGSize) {
        self.key = key
        self.gameId = gameId
        self.levelId = levelId
        if levelId.hasPrefix("level-"), let n = Int32(levelId.dropFirst(6)) {
            self.levelNum = n
        } else {
            self.levelNum = 1
        }
        let w = max(320, Float(viewportSize.width))
        let h = max(240, Float(viewportSize.height))
        self.handle = oa_gb_alloc(w, h)
        if let hnd = self.handle {
            oa_gb_set_world_attached(hnd, 1)
            oa_gb_set_claims_clock(hnd, Int64(Date().timeIntervalSince1970))
        }
        try? FileManager.default.createDirectory(at: Self.levelsDir, withIntermediateDirectories: true)
        loadInitialContent()
        hydrateLocalPacksIfAvailable()
    }

    deinit {
        stopAttachServer()
        if let h = handle { oa_gb_free(h) }
    }

    func knownLevels() -> [Int32] {
        var nums: Set<Int32> = [levelNum]
        if let entries = try? FileManager.default.contentsOfDirectory(atPath: Self.levelsDir.path) {
            for e in entries where e.hasPrefix("level-") && e.hasSuffix(".world") {
                let stem = e.dropFirst(6).dropLast(6)
                if let n = Int32(stem) { nums.insert(n) }
            }
        }
        return nums.sorted()
    }

    private func levelFileURL(_ num: Int32) -> URL {
        if num == 0 {
            return Self.levelsDir.appendingPathComponent("\(gameId.isEmpty ? "scrappers-arena" : gameId).world")
        }
        return Self.levelsDir.appendingPathComponent("level-\(num).world")
    }

    private func loadInitialContent() {
        guard let h = handle else { return }
        if gameId == "scrappers-arena" {
            let url = levelFileURL(0)
            let data = (try? Data(contentsOf: url)) ?? Data()
            data.withUnsafeBytes { ptr in
                _ = oa_gb_load_game_buf(h, 0, ptr.bindMemory(to: UInt8.self).baseAddress, Int32(data.count))
            }
        } else if gameId == "splicebots" {
            let url = levelFileURL(0)
            let data = (try? Data(contentsOf: url)) ?? Data()
            data.withUnsafeBytes { ptr in
                _ = oa_gb_load_game_buf(h, 1, ptr.bindMemory(to: UInt8.self).baseAddress, Int32(data.count))
            }
        } else {
            let url = levelFileURL(levelNum)
            if let data = try? Data(contentsOf: url), !data.isEmpty {
                data.withUnsafeBytes { ptr in
                    _ = oa_gb_load_level_buf(h, levelNum, ptr.bindMemory(to: UInt8.self).baseAddress, Int32(data.count))
                }
            } else {
                _ = runVerb("new-level", argsJson: "")
            }
        }
        flushSaveIfDue()
    }

    /// Locates hydrated GameBuilder binary assets (scrappers.gbm, splice-bots-jam.pack) in ~/.openagents/assets or repo.
    func hydrateLocalPacksIfAvailable() {
        guard let h = handle, let device = MetaverseGpuCoordinator.shared.device else { return }
        let fm = FileManager.default
        let home = fm.homeDirectoryForCurrentUser
        let gbmCandidates: [URL] = [
            home.appendingPathComponent(".openagents/assets/gamebuilder/scrappers.gbm"),
            Bundle.main.resourceURL?.appendingPathComponent("GameBuilder/scrappers.gbm"),
            URL(fileURLWithPath: FileManager.default.currentDirectoryPath).appendingPathComponent("gamebuilder/assets/scrappers.gbm"),
        ].compactMap { $0 }

        var foundGbm: Data?
        for c in gbmCandidates where fm.fileExists(atPath: c.path) {
            if let d = try? Data(contentsOf: c), d.count > 8 { foundGbm = d; break }
        }
        if let gbm = foundGbm {
            let ok = gbm.withUnsafeBytes { ptr in
                oa_gb_load_gbm(h, ptr.bindMemory(to: UInt8.self).baseAddress, Int32(gbm.count))
            }
            if ok != 0 {
                let dataOff = Int(oa_gb_get_mesh_vertices_at(h))
                let vertBytes = gbm.count - dataOff
                if dataOff > 0 && vertBytes > 0 {
                    gbmVertexBuffer = gbm.withUnsafeBytes { ptr in
                        device.makeBuffer(bytes: ptr.baseAddress!.advanced(by: dataOff), length: vertBytes, options: .storageModeShared)
                    }
                    let mc = oa_gb_get_mesh_count(h)
                    var rng = [Int32](repeating: 0, count: 2)
                    gbmMeshRanges.removeAll()
                    for id in Int32(1)...max(1, mc) {
                        if oa_gb_get_mesh_range(h, id, &rng) != 0 {
                            gbmMeshRanges[id] = (Int(rng[0]), Int(rng[1]))
                        }
                    }
                    gbmLoaded = true
                }
            }
        } else {
            needsHydration = true
        }

        // Optional SPLICEBOTS UE jam world pack (farm.coax pipeline)
        if gameId == "splicebots" {
            let packCandidates: [URL] = [
                home.appendingPathComponent(".openagents/assets/gamebuilder/splice-bots-jam.pack"),
                home.appendingPathComponent(".openagents/assets/splice-bots-jam.pack"),
            ]
            for c in packCandidates where fm.fileExists(atPath: c.path) {
                if let d = try? Data(contentsOf: c), d.count > 32 {
                    let ok = d.withUnsafeBytes { ptr in
                        oa_gb_load_farm_pack(h, ptr.bindMemory(to: UInt8.self).baseAddress, Int32(d.count))
                    }
                    if ok != 0 { farmLoaded = true; break }
                }
            }
        }
    }

    /// Dispatches ANY GameBuilder action strictly through GB_COMMANDS (ed_command).
    @discardableResult
    func runVerb(_ verb: String, argsJson: String) -> (ok: Bool, json: String) {
        guard let h = handle else { return (false, "{\"ok\":false,\"why\":\"no editor\"}") }
        oa_gb_set_claims_clock(h, Int64(Date().timeIntervalSince1970))
        let lv = knownLevels()
        var out = [CChar](repeating: 0, count: 65536)
        let rc: Int32 = lv.withUnsafeBufferPointer { buf in
            oa_gb_exec_verb(h, verb, argsJson, buf.baseAddress, Int32(buf.count), &out, Int32(out.count))
        }
        let json = String(cString: out)
        handleOpenDueIfAny()
        flushSaveIfDue()
        return (rc > 0, json)
    }

    /// Dispatches a GB_CONTROLS button press strictly through ed_control_press_with.
    @discardableResult
    func pressControl(_ ctrlId: String, argsJson: String = "") -> (ok: Bool, json: String) {
        guard let h = handle else { return (false, "") }
        let lv = knownLevels()
        var out = [CChar](repeating: 0, count: 65536)
        let rc: Int32 = lv.withUnsafeBufferPointer { buf in
            oa_gb_exec_ctrl(h, ctrlId, argsJson, buf.baseAddress, Int32(buf.count), &out, Int32(out.count))
        }
        let json = String(cString: out)
        handleOpenDueIfAny()
        flushSaveIfDue()
        return (rc > 0, json)
    }

    private func handleOpenDueIfAny() {
        guard let h = handle else { return }
        let openLv = oa_gb_poll_open_due(h)
        if openLv >= 0 {
            levelNum = openLv
            levelId = openLv == 0 ? gameId : "level-\(openLv)"
            let url = levelFileURL(openLv)
            if let data = try? Data(contentsOf: url), !data.isEmpty {
                data.withUnsafeBytes { ptr in
                    _ = oa_gb_load_level_buf(h, openLv, ptr.bindMemory(to: UInt8.self).baseAddress, Int32(data.count))
                }
            }
        }
        let openGame = oa_gb_poll_open_game_due(h)
        if openGame >= 0 {
            gameId = openGame == 1 ? "splicebots" : "scrappers-arena"
            levelNum = 0
            levelId = gameId
            let data = (try? Data(contentsOf: levelFileURL(0))) ?? Data()
            data.withUnsafeBytes { ptr in
                _ = oa_gb_load_game_buf(h, openGame, ptr.bindMemory(to: UInt8.self).baseAddress, Int32(data.count))
            }
        }
    }

    func flushSaveIfDue() {
        guard let h = handle else { return }
        var buf = [UInt8](repeating: 0, count: 262144)
        // Persist extra levels created by multi-level templates (e.g. crates creates 3 puzzle rooms)
        var extraGuard = 0
        while extraGuard < 16 {
            let extraLv = oa_gb_poll_more_due(h)
            if extraLv <= 0 { break }
            let m = Int(oa_gb_save_more_buf(h, &buf, Int32(buf.count)))
            if m > 0 {
                try? FileManager.default.createDirectory(at: Self.levelsDir, withIntermediateDirectories: true)
                try? Data(buf[0..<m]).write(to: levelFileURL(extraLv), options: .atomic)
            } else {
                break
            }
            extraGuard += 1
        }
        guard oa_gb_is_save_due(h) != 0 else { return }
        let n = Int(oa_gb_save_level_buf(h, &buf, Int32(buf.count)))
        if n > 0 {
            let snap = snapshotUi()
            levelNum = snap.levelNum
            let url = levelFileURL(levelNum)
            try? FileManager.default.createDirectory(at: Self.levelsDir, withIntermediateDirectories: true)
            try? Data(buf[0..<n]).write(to: url, options: .atomic)
            oa_gb_clear_save_due(h)
        }
    }

    func snapshotUi() -> GameBuilderUiSnapshot {
        var ints = [Int32](repeating: 0, count: 26)
        var cam = [Int64](repeating: 0, count: 9)
        var lbl = [CChar](repeating: 0, count: 64)
        var note = [CChar](repeating: 0, count: 256)
        if let h = handle {
            _ = oa_gb_snapshot_ui(h, &ints, &cam, &lbl, Int32(lbl.count), &note, Int32(note.count))
        }
        return GameBuilderUiSnapshot(
            tool: ints[0],
            canUndo: ints[1] != 0,
            canRedo: ints[2] != 0,
            templateId: ints[3],
            playing: ints[4] != 0,
            levelNum: ints[5],
            savedRev: ints[6],
            saveDue: ints[7] != 0,
            readOnly: ints[8] != 0,
            changeCount: ints[9],
            selCount: ints[10],
            selId: ints[11],
            selKind: ints[12],
            fields: Array(ints[13..<24]),
            actorCount: ints[24],
            claimsCount: ints[25],
            selLabel: String(cString: lbl),
            statusNote: String(cString: note),
            camCoordsCm: cam
        )
    }

    func treeTSV() -> String {
        guard let h = handle else { return "" }
        var buf = [CChar](repeating: 0, count: 16384)
        _ = oa_gb_get_tree_tsv(h, &buf, Int32(buf.count))
        return String(cString: buf)
    }

    func parsePlaytestResult(_ json: String) {
        lastPlaytestJson = json
        playtestPins.removeAll()
        guard let data = json.data(using: .utf8),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            lastPlaytestSummary = "Play-test failed to parse"
            return
        }
        let bot = obj["bot"] as? String ?? "match"
        if let stuck = obj["stuck"] as? [[String: Any]] {
            for s in stuck {
                let seat = s["seat"] as? Int ?? 0
                let times = s["times"] as? Int ?? 1
                let x = Int32(s["x"] as? Int ?? 0)
                let z = Int32(s["z"] as? Int ?? 0)
                playtestPins.append(.init(label: "Stuck bot #\(seat) (\(times)x)", xCm: x, zCm: z))
            }
        }
        if let unreached = obj["unreached"] as? [[String: Any]] {
            for u in unreached {
                let kind = u["kind"] as? String ?? "marker"
                let id = u["id"] as? Int ?? 0
                let x = Int32(u["x"] as? Int ?? 0)
                let z = Int32(u["z"] as? Int ?? 0)
                playtestPins.append(.init(label: "Unreached \(kind) #\(id)", xCm: x, zCm: z))
            }
        }
        if bot == "walk-through" {
            let reached = obj["reached"] as? Int ?? 0
            let targets = obj["targets"] as? Int ?? 0
            lastPlaytestSummary = "Walk-through: \(reached)/\(targets) reached · \(playtestPins.count) issues"
        } else {
            let score = obj["score"] as? [Int] ?? [0, 0]
            let winner = obj["winner"] as? String ?? "draw"
            let s0 = score.count > 0 ? score[0] : 0
            let s1 = score.count > 1 ? score[1] : 0
            lastPlaytestSummary = "Match \(s0)–\(s1) (\(winner)) · \(playtestPins.count) issues"
        }
    }

    // MARK: - Live --attach Unix Socket (owner-only 0600, peer UID verified by cxsock_peer_self)

    @discardableResult
    func startAttachServer() -> Bool {
        guard let h = handle, !attachListening else { return attachListening }
        try? FileManager.default.createDirectory(at: Self.baseDir, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        try? FileManager.default.createDirectory(at: Self.levelsDir, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        let sockURL = Self.baseDir.appendingPathComponent("attach.sock")
        try? FileManager.default.removeItem(at: sockURL)
        guard oa_gb_listen_attach(h, sockURL.path) != 0 else { return false }
        chmod(sockURL.path, 0o600)
        attachSocketPath = sockURL.path
        attachListening = true
        // Write Levels/attach pointer file so `gamebuilder --attach` & `gamebuilder mcp --attach` auto-discover us
        let ptrURL = Self.levelsDir.appendingPathComponent("attach")
        try? (sockURL.path + "\n").write(to: ptrURL, atomically: true, encoding: .utf8)
        chmod(ptrURL.path, 0o600)
        return true
    }

    func pollAttach() -> Bool {
        guard let h = handle, attachListening else { return false }
        let lv = knownLevels()
        let ran: Int32 = lv.withUnsafeBufferPointer { buf in
            oa_gb_poll_attach(h, buf.baseAddress, Int32(buf.count))
        }
        if ran != 0 {
            attachRequestsServed &+= 1
            handleOpenDueIfAny()
            flushSaveIfDue()
            return true
        }
        return false
    }

    func stopAttachServer() {
        guard attachListening else { return }
        if let h = handle { oa_gb_close_attach(h) }
        if !attachSocketPath.isEmpty {
            try? FileManager.default.removeItem(atPath: attachSocketPath)
        }
        attachListening = false
    }
}

#endif
