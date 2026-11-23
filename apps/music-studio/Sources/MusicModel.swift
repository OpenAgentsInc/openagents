// Firm reason: 6 (macOS Swift state wrapper over Coax C ABI music-bridge.h + JSON hot-reload).
import AppKit
import Foundation

struct TrackViewItem {
    let index: Int32
    let name: String
    let section: UInt8
    let midiChannel: UInt8
    let gmProgram: UInt8
    let transposePreview: Int8
    let mute: Bool
    let solo: Bool
    let ghostVisible: Bool
    let gain: Float
    let pan: Float
    let reverbSend: Float
}

struct NoteViewItem {
    let id: UInt32
    let slot: Int32
    let track: UInt8
    let pitch: UInt8
    let velocity: UInt8
    let startTick: Int64
    let durTick: Int64
}

struct MarkerViewItem {
    let index: Int32
    let tick: Int64
    let tonic: UInt8
    let minor: Bool
    let num: UInt8
    let den: UInt8
    let sectionSlot: UInt8
    let label: String
}

enum StudioPalette {
    static let bgCanvas      = NSColor(red: 0.045, green: 0.047, blue: 0.058, alpha: 1)
    static let bgPanel       = NSColor(red: 0.070, green: 0.074, blue: 0.090, alpha: 1)
    static let bgHeader      = NSColor(red: 0.088, green: 0.092, blue: 0.112, alpha: 1)
    static let bgCard        = NSColor(red: 0.110, green: 0.116, blue: 0.142, alpha: 1)
    static let border        = NSColor(red: 0.18, green: 0.19, blue: 0.24, alpha: 1)
    static let textPrimary   = NSColor(red: 0.94, green: 0.95, blue: 0.98, alpha: 1)
    static let textMuted     = NSColor(red: 0.56, green: 0.59, blue: 0.67, alpha: 1)
    static let accentGold    = NSColor(red: 0.96, green: 0.76, blue: 0.30, alpha: 1)
    static let accentCrimson = NSColor(red: 0.90, green: 0.26, blue: 0.36, alpha: 1)
    static let playhead      = NSColor(red: 1.00, green: 0.84, blue: 0.36, alpha: 0.95)

    static func sectionColor(_ section: UInt8) -> NSColor {
        switch section {
        case 0: return NSColor(red: 0.96, green: 0.74, blue: 0.28, alpha: 1) // Keys / Piano (Gold)
        case 1: return NSColor(red: 0.90, green: 0.24, blue: 0.36, alpha: 1) // Gothic Pipe Organ (Crimson)
        case 2: return NSColor(red: 0.30, green: 0.72, blue: 0.98, alpha: 1) // Strings (Sky Cyan)
        case 3: return NSColor(red: 0.28, green: 0.85, blue: 0.62, alpha: 1) // Woodwinds (Emerald)
        case 4: return NSColor(red: 0.98, green: 0.56, blue: 0.24, alpha: 1) // Brass (Amber Orange)
        case 5: return NSColor(red: 0.36, green: 0.88, blue: 0.86, alpha: 1) // Harp (Teal)
        case 6: return NSColor(red: 0.84, green: 0.42, blue: 0.96, alpha: 1) // Percussion (Violet)
        case 7: return NSColor(red: 0.96, green: 0.48, blue: 0.72, alpha: 1) // Choir / Celesta (Rose)
        default: return NSColor(red: 0.70, green: 0.78, blue: 0.95, alpha: 1)
        }
    }

    static let gmPresets: [(program: UInt8, name: String)] = [
        (0, "000 Concert Grand Piano"),
        (4, "004 Electric Piano 1 (Rhodes)"),
        (5, "005 Electric Piano 2 (FM)"),
        (6, "006 Harpsichord"),
        (8, "008 Celesta"),
        (14, "014 Tubular Bells"),
        (16, "016 Drawbar Organ"),
        (19, "019 Church / Pipe Organ"),
        (20, "020 Reed Organ"),
        (40, "040 Solo Violin"),
        (41, "041 Solo Viola"),
        (42, "042 Solo Cello"),
        (43, "043 Contrabass"),
        (44, "044 Tremolo Strings"),
        (45, "045 Pizzicato Strings"),
        (46, "046 Concert Harp"),
        (47, "047 Timpani"),
        (48, "048 String Ensemble 1"),
        (49, "049 Slow Strings Ensemble 2"),
        (52, "052 Choir Aahs"),
        (53, "053 Voice Oohs"),
        (55, "055 Orchestra Hit"),
        (56, "056 Trumpet"),
        (57, "057 Trombone"),
        (58, "058 Tuba"),
        (60, "060 French Horn"),
        (61, "061 Brass Section"),
        (68, "068 Oboe"),
        (69, "069 English Horn"),
        (70, "070 Bassoon"),
        (71, "071 Clarinet"),
        (72, "072 Piccolo"),
        (73, "073 Flute"),
        (33, "033 Electric Bass (Finger)"),
        (81, "081 Synth Saw Lead"),
        (89, "089 Warm Synth Pad"),
    ]

    static func pitchName(_ pitch: Int) -> String {
        let names = ["C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"]
        let oct = (pitch / 12) - 1
        return "\(names[((pitch % 12) + 12) % 12])\(oct)"
    }

    static func keyName(tonic: UInt8, minor: Bool) -> String {
        let majorNames = ["C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B"]
        let minorNames = ["C", "C#", "D", "Eb", "E", "F", "F#", "G", "G#", "A", "Bb", "B"]
        let i = Int(tonic % 12)
        return minor ? "\(minorNames[i]) Min" : "\(majorNames[i]) Maj"
    }
}

final class StudioStore {
    let handle: OpaquePointer
    let lock = NSRecursiveLock()

    var selectedTrack: Int32 = 0
    var selectedNoteSlots: Set<Int32> = []
    var snapTicks: Int64 = 240            // 1/16th note (960 / 4)
    var loopEnabled: Bool = false
    var loopStartTick: Int64 = 0
    var loopEndTick: Int64 = 8 * 3840
    var activeMovement: Int = 1           // 1..5
    var mvt4TransposedToDm: Bool = false

    // Live software keyboard pressed notes (MIDI 0..127)
    var liveInputPitches: Set<UInt8> = []

    // JSON hot-reload watcher
    var liveJsonURL: URL {
        let dir = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".openagents/music/projects", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir.appendingPathComponent("live.oamusic.json")
    }
    var lastLiveJsonMtime: Date?
    var hotReloadEnabled: Bool = false

    var onChange: (() -> Void)?
    var onStatus: ((String) -> Void)?

    init() {
        guard let h = ms_create() else { fatalError("Failed to allocate Coax MusicProject") }
        self.handle = h
    }

    deinit {
        ms_destroy(handle)
    }

    func withLock<T>(_ body: () -> T) -> T {
        lock.lock()
        defer { lock.unlock() }
        return body()
    }

    func tracks() -> [TrackViewItem] {
        withLock {
            let n = ms_track_count(handle)
            var out: [TrackViewItem] = []
            out.reserveCapacity(Int(n))
            for i in 0..<n {
                var t = MsTrackInfo()
                if ms_get_track(handle, i, &t), t.active != 0 {
                    let name = withUnsafePointer(to: &t.name) {
                        $0.withMemoryRebound(to: CChar.self, capacity: 49) { String(cString: $0) }
                    }
                    out.append(TrackViewItem(
                        index: i, name: name, section: t.section, midiChannel: t.midi_channel,
                        gmProgram: t.gm_program, transposePreview: t.transpose_preview,
                        mute: t.mute != 0, solo: t.solo != 0, ghostVisible: t.ghost_visible != 0,
                        gain: t.gain, pan: t.pan, reverbSend: t.reverb_send
                    ))
                }
            }
            return out
        }
    }

    func notes() -> [NoteViewItem] {
        withLock {
            let cap = ms_note_capacity(handle)
            var out: [NoteViewItem] = []
            out.reserveCapacity(Int(cap))
            for i in 0..<cap {
                var n = MsNoteInfo()
                if ms_get_note(handle, i, &n), n.active != 0 {
                    out.append(NoteViewItem(
                        id: n.id, slot: n.slot, track: n.track, pitch: n.pitch,
                        velocity: n.velocity, startTick: n.start_tick, durTick: n.dur_tick
                    ))
                }
            }
            return out
        }
    }

    func markers() -> [MarkerViewItem] {
        withLock {
            let n = ms_marker_count(handle)
            var out: [MarkerViewItem] = []
            for i in 0..<n {
                var m = MsMarkerInfo()
                if ms_get_marker(handle, i, &m) {
                    let label = withUnsafePointer(to: &m.label) {
                        $0.withMemoryRebound(to: CChar.self, capacity: 49) { String(cString: $0) }
                    }
                    out.append(MarkerViewItem(
                        index: i, tick: m.tick, tonic: m.tonic, minor: m.minor != 0,
                        num: m.num, den: m.den, sectionSlot: m.section_slot, label: label
                    ))
                }
            }
            return out
        }
    }

    func movementRange(slot: Int) -> (startTick: Int64, endTick: Int64) {
        let ms = markers()
        if let idx = ms.firstIndex(where: { Int($0.sectionSlot) == slot }) {
            let start = ms[idx].tick
            let end = (idx + 1 < ms.count) ? ms[idx + 1].tick : max(start + 8 * 3840, withLock { ms_total_ticks(handle) })
            return (start, end)
        }
        let s = Int64(max(0, slot - 1) * 8) * 3840
        return (s, s + 8 * 3840)
    }

    func activePitchesByTrack() -> [UInt8: [UInt8]] {
        withLock {
            let n = ms_track_count(handle)
            var result: [UInt8: [UInt8]] = [:]
            for t in 0..<n {
                let lo = ms_active_lo(handle, t)
                let hi = ms_active_hi(handle, t)
                if lo == 0 && hi == 0 { continue }
                var arr: [UInt8] = []
                for bit in 0..<64 {
                    if (lo & (UInt64(1) << UInt64(bit))) != 0 { arr.append(UInt8(bit)) }
                    if (hi & (UInt64(1) << UInt64(bit))) != 0 { arr.append(UInt8(64 + bit)) }
                }
                result[UInt8(t)] = arr
            }
            return result
        }
    }

    func importMidiFile(_ url: URL, mergeAtTick: Int64) -> Int32 {
        guard let data = try? Data(contentsOf: url), !data.isEmpty else {
            onStatus?("Could not read \(url.lastPathComponent)")
            return -1
        }
        let title = url.deletingPathExtension().lastPathComponent
        let imported: Int32 = withLock {
            data.withUnsafeBytes { raw -> Int32 in
                guard let base = raw.baseAddress?.assumingMemoryBound(to: UInt8.self) else { return -1 }
                return ms_smf_import(handle, base, Int32(data.count), mergeAtTick, title)
            }
        }
        if imported >= 0 {
            if mergeAtTick < 0 {
                selectedTrack = 0
                selectedNoteSlots.removeAll()
            }
            onStatus?(mergeAtTick < 0
                ? "Imported \(imported) notes from \(url.lastPathComponent)"
                : "Merged \(imported) notes from \(url.lastPathComponent) at bar \(mergeAtTick / 3840 + 1)")
            onChange?()
        } else {
            onStatus?("Invalid or unsupported MIDI file: \(url.lastPathComponent)")
        }
        return imported
    }

    func exportMidiFile(_ url: URL) -> Bool {
        let cap = 2 * 1024 * 1024
        var buf = Data(count: cap)
        let written: Int32 = withLock {
            buf.withUnsafeMutableBytes { raw -> Int32 in
                guard let base = raw.baseAddress?.assumingMemoryBound(to: UInt8.self) else { return -1 }
                return ms_smf_export(handle, base, Int32(cap))
            }
        }
        guard written > 0 else { return false }
        buf.count = Int(written)
        do {
            try buf.write(to: url, options: .atomic)
            onStatus?("Exported \(written) bytes to \(url.lastPathComponent)")
            return true
        } catch {
            onStatus?("Failed to save MIDI: \(error.localizedDescription)")
            return false
        }
    }

    // MARK: - Live JSON Export & Hot-Reload (for programmatic scripting & AI agent remixing)

    struct JsonProject: Codable {
        struct JTrack: Codable {
            let name: String
            let section: UInt8
            let gm_program: UInt8
            let midi_channel: UInt8
            let pan: Float
            let reverb_send: Float
            let gain: Float
            let mute: Bool
            let solo: Bool
            let transpose_preview: Int8
        }
        struct JNote: Codable {
            let track: UInt8
            let pitch: UInt8
            let velocity: UInt8
            let start_tick: Int64
            let dur_tick: Int64
        }
        let title: String
        let tracks: [JTrack]
        let notes: [JNote]
    }

    func saveLiveJson() {
        let trs = tracks().map {
            JsonProject.JTrack(
                name: $0.name, section: $0.section, gm_program: $0.gmProgram,
                midi_channel: $0.midiChannel, pan: $0.pan, reverb_send: $0.reverbSend,
                gain: $0.gain, mute: $0.mute, solo: $0.solo, transpose_preview: $0.transposePreview
            )
        }
        let nts = notes().map {
            JsonProject.JNote(track: $0.track, pitch: $0.pitch, velocity: $0.velocity, start_tick: $0.startTick, dur_tick: $0.durTick)
        }
        let doc = JsonProject(title: "Live Studio Project", tracks: trs, notes: nts)
        let enc = JSONEncoder()
        enc.outputFormatting = [.prettyPrinted, .sortedKeys]
        if let data = try? enc.encode(doc) {
            try? data.write(to: liveJsonURL, options: .atomic)
            lastLiveJsonMtime = (try? FileManager.default.attributesOfItem(atPath: liveJsonURL.path)[.modificationDate]) as? Date
            onStatus?("Saved live JSON -> ~/.openagents/music/projects/live.oamusic.json")
        }
    }

    func pollHotReloadIfNeeded() -> Bool {
        guard hotReloadEnabled else { return false }
        guard let attrs = try? FileManager.default.attributesOfItem(atPath: liveJsonURL.path),
              let mtime = attrs[.modificationDate] as? Date else { return false }
        if let prev = lastLiveJsonMtime, mtime <= prev { return false }
        lastLiveJsonMtime = mtime
        guard let data = try? Data(contentsOf: liveJsonURL),
              let doc = try? JSONDecoder().decode(JsonProject.self, from: data) else { return false }
        withLock {
            ms_init_empty(handle, doc.title)
            for t in doc.tracks {
                let idx = ms_add_track(handle, t.name, t.section, t.gm_program, t.midi_channel, t.pan, t.reverb_send)
                if idx >= 0 {
                    ms_set_track_gain(handle, idx, t.gain)
                    ms_set_track_mute(handle, idx, t.mute)
                    ms_set_track_solo(handle, idx, t.solo)
                    ms_set_track_transpose(handle, idx, t.transpose_preview)
                }
            }
            for n in doc.notes {
                _ = ms_add_note(handle, n.track, n.pitch, n.velocity, n.start_tick, n.dur_tick)
            }
        }
        onStatus?("Hot-reloaded \(doc.notes.count) notes from live.oamusic.json")
        onChange?()
        return true
    }

    // MARK: - Programmatic Remix Command Interpreter (drives Coax music/remix.coax directly)

    func runRemixCommand(_ raw: String) -> String {
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty { return "" }
        let parts = trimmed.split(whereSeparator: { $0.isWhitespace }).map(String.init)
        let cmd = parts[0].lowercased()

        func argInt(_ key: String, default def: Int) -> Int {
            for p in parts.dropFirst() {
                if p.lowercased().hasPrefix("\(key)="), let v = Int(p.dropFirst(key.count + 1)) { return v }
            }
            return def
        }
        func barRange() -> (Int64, Int64) {
            for p in parts.dropFirst() {
                if p.lowercased().hasPrefix("bars=") {
                    let rhs = String(p.dropFirst(5))
                    let ab = rhs.split(separator: "-").compactMap { Int($0) }
                    if ab.count == 2 {
                        return (Int64(max(0, ab[0] - 1)) * 3840, Int64(max(ab[0], ab[1])) * 3840)
                    }
                }
            }
            return (0, 0)
        }

        let (b0, b1) = barRange()
        switch cmd {
        case "transpose":
            let tr = Int32(argInt("track", default: Int(selectedTrack)))
            let semi = Int32(argInt("semi", default: -3))
            let n = withLock { ms_remix_transpose(handle, tr, b0, b1, semi) }
            onChange?()
            return "Transposed \(n) notes on track \(tr) by \(semi >= 0 ? "+\(semi)" : "\(semi)") semitones"

        case "quantize":
            let tr = Int32(argInt("track", default: Int(selectedTrack)))
            let grid = Int64(argInt("grid", default: 240))
            let str = Int32(argInt("strength", default: 100))
            let n = withLock { ms_remix_quantize(handle, tr, b0, b1, grid, str) }
            onChange?()
            return "Quantized \(n) notes on track \(tr) to \(grid) ticks"

        case "humanize":
            let tr = Int32(argInt("track", default: Int(selectedTrack)))
            let tk = Int32(argInt("ticks", default: 18))
            let vel = Int32(argInt("vel", default: 8))
            let seed = UInt32(argInt("seed", default: Int.random(in: 1...99999)))
            let n = withLock { ms_remix_humanize(handle, tr, b0, b1, tk, vel, seed) }
            onChange?()
            return "Humanized \(n) notes on track \(tr) (±\(tk) ticks, ±\(vel) vel)"

        case "double":
            let src = Int32(argInt("src", default: Int(selectedTrack)))
            let dst = Int32(argInt("dst", default: Int(selectedTrack)))
            let semi = Int32(argInt("semi", default: -12))
            let vel = Int32(argInt("vel", default: 85))
            let n = withLock { ms_remix_copy_layer(handle, src, dst, b0, b1, 0, semi, vel) }
            onChange?()
            return "Doubled \(n) notes track \(src) -> \(dst) (\(semi) st)"

        case "explode":
            let src = Int32(argInt("src", default: Int(selectedTrack)))
            var dst: [UInt8] = [2, 3, 4]
            for p in parts.dropFirst() where p.lowercased().hasPrefix("dst=") {
                let parsed = p.dropFirst(4).split(separator: ",").compactMap { UInt8($0) }
                if !parsed.isEmpty { dst = parsed }
            }
            let n = withLock { dst.withUnsafeBufferPointer { ms_remix_explode(handle, src, b0, b1, $0.baseAddress, Int32($0.count)) } }
            onChange?()
            return "Exploded \(n) chord voices from track \(src) across tracks \(dst)"

        case "swell":
            let tr = Int32(argInt("track", default: Int(selectedTrack)))
            let v0 = Int32(argInt("v0", default: 58))
            let vp = Int32(argInt("peak", default: 118))
            let v1 = Int32(argInt("v1", default: 74))
            let endT = b1 > b0 ? b1 : withLock { ms_total_ticks(handle) }
            let n = withLock { ms_remix_swell(handle, tr, b0, endT, v0, vp, v1) }
            onChange?()
            return "Applied velocity swell (\(v0)->\(vp)->\(v1)) to \(n) notes on track \(tr)"

        case "tempo":
            let bar = max(1, argInt("bar", default: 1))
            let bpm = max(20, min(400, argInt("bpm", default: 120)))
            let ramp = parts.contains(where: { $0.lowercased() == "ramp=1" || $0.lowercased() == "ramp=true" })
            let ok = withLock { ms_set_tempo(handle, Int64(bar - 1) * 3840, Int32(bpm * 1000), ramp) }
            onChange?()
            return ok ? "Set tempo at bar \(bar) to \(bpm) BPM (ramp=\(ramp))" : "Failed to set tempo"

        case "pit":
            withLock { ms_remix_pit_seating(handle) }
            onChange?()
            return "Applied Broadway 29-piece pit-orchestra panning & hall reverb sends"

        case "merge":
            guard parts.count >= 2 else { return "Usage: merge /path/to/file.mid bar=17" }
            let path = (parts[1] as NSString).expandingTildeInPath
            let bar = max(1, argInt("bar", default: 1))
            let n = importMidiFile(URL(fileURLWithPath: path), mergeAtTick: Int64(bar - 1) * 3840)
            return n >= 0 ? "Merged \(n) notes at bar \(bar)" : "Failed to merge \(path)"

        case "undo":
            let ok = withLock { ms_undo(handle) }
            onChange?()
            return ok ? "Undid last edit batch" : "Nothing to undo"

        case "redo":
            let ok = withLock { ms_redo(handle) }
            onChange?()
            return ok ? "Redid edit batch" : "Nothing to redo"

        default:
            return "Commands: transpose | quantize | humanize | double | explode | swell | tempo | pit | merge <file.mid> bar=N | undo | redo"
        }
    }
}
