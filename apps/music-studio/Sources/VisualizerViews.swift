// Firm reason: 6 (macOS built-in 88-key interactive keyboard & 60 fps Falling-Notes cascade visualizer).
import AppKit
import Foundation

// Shared 88-key geometry helper (A0=21 .. C8=108 -> 52 white keys).
enum Piano88Geometry {
    static let minPitch = 21
    static let maxPitch = 108

    static let whitePitches: [Int] = (minPitch...maxPitch).filter { !isBlack($0) }
    static let blackPitches: [Int] = (minPitch...maxPitch).filter { isBlack($0) }

    static func isBlack(_ pitch: Int) -> Bool {
        let m = ((pitch % 12) + 12) % 12
        return m == 1 || m == 3 || m == 6 || m == 8 || m == 10
    }

    static func rect(forPitch pitch: Int, width: CGFloat, height: CGFloat) -> CGRect {
        let whiteW = width / CGFloat(whitePitches.count)
        if !isBlack(pitch) {
            let idx = whitePitches.firstIndex(of: pitch) ?? 0
            return CGRect(x: CGFloat(idx) * whiteW, y: 0, width: whiteW, height: height)
        }
        // Position black key slightly straddling the white key to its left.
        let prevIdx = whitePitches.firstIndex(of: pitch - 1) ?? 0
        let bw = whiteW * 0.64
        let bx = (CGFloat(prevIdx) + 1.0) * whiteW - bw * 0.5
        return CGRect(x: bx, y: 0, width: bw, height: height * 0.62)
    }
}

// MARK: - Interactive Horizontal 88-Key Playable Keyboard (bottom dock)

final class SoftwareKeyboardView: NSView {
    let store: StudioStore
    let audio: StudioAudioEngine
    var qwertyBaseOctave: Int = 4       // C4 = MIDI 60
    private var draggingPitch: UInt8?

    override var isFlipped: Bool { true }

    // QWERTY key hints for middle octave
    private let qwertyLabels: [Int: String] = [
        0: "A", 1: "W", 2: "S", 3: "E", 4: "D", 5: "F", 6: "T",
        7: "G", 8: "Y", 9: "H", 10: "U", 11: "J", 12: "K",
    ]

    init(store: StudioStore, audio: StudioAudioEngine) {
        self.store = store
        self.audio = audio
        super.init(frame: .zero)
        wantsLayer = true
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) not supported") }

    override func draw(_ dirtyRect: NSRect) {
        guard let ctx = NSGraphicsContext.current?.cgContext else { return }
        ctx.setFillColor(StudioPalette.bgPanel.cgColor)
        ctx.fill(bounds)

        let tracks = store.tracks()
        let trackMap = Dictionary(uniqueKeysWithValues: tracks.map { ($0.index, $0) })
        let activeByTrack = store.activePitchesByTrack()

        var pitchColor: [Int: NSColor] = [:]
        for (trIdx, pitches) in activeByTrack {
            let sec = trackMap[Int32(trIdx)]?.section ?? 0
            let col = StudioPalette.sectionColor(sec)
            for p in pitches { pitchColor[Int(p)] = col }
        }
        for p in store.liveInputPitches {
            pitchColor[Int(p)] = StudioPalette.accentGold
        }

        let w = bounds.width
        let h = bounds.height
        let baseC = (qwertyBaseOctave + 1) * 12

        let cAttrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.monospacedSystemFont(ofSize: 8.5, weight: .semibold),
            .foregroundColor: NSColor(white: 0.36, alpha: 1),
        ]
        let qAttrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.monospacedSystemFont(ofSize: 9, weight: .bold),
            .foregroundColor: NSColor(red: 0.25, green: 0.30, blue: 0.42, alpha: 0.85),
        ]
        let qBlackAttrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.monospacedSystemFont(ofSize: 8, weight: .bold),
            .foregroundColor: NSColor(white: 0.72, alpha: 0.9),
        ]

        // 1. White keys
        for p in Piano88Geometry.whitePitches {
            let r = Piano88Geometry.rect(forPitch: p, width: w, height: h)
            let keyR = CGRect(x: r.minX + 0.5, y: 1, width: max(2, r.width - 1), height: h - 2)
            let path = CGPath(roundedRect: keyR, cornerWidth: 2, cornerHeight: 2, transform: nil)
            if let col = pitchColor[p] {
                ctx.saveGState()
                ctx.setShadow(offset: .zero, blur: 8, color: col.withAlphaComponent(0.9).cgColor)
                ctx.setFillColor(col.cgColor)
                ctx.addPath(path)
                ctx.fillPath()
                ctx.restoreGState()
            } else {
                ctx.setFillColor(NSColor(red: 0.91, green: 0.92, blue: 0.95, alpha: 1).cgColor)
                ctx.addPath(path)
                ctx.fillPath()
            }

            if ((p % 12) + 12) % 12 == 0 {
                let lbl = "C\((p / 12) - 1)" as NSString
                lbl.draw(at: NSPoint(x: keyR.minX + 3, y: h - 14), withAttributes: cAttrs)
            }
            if let q = qwertyLabels[p - baseC] {
                (q as NSString).draw(at: NSPoint(x: keyR.midX - 3.5, y: h - 28), withAttributes: qAttrs)
            }
        }

        // 2. Black keys on top
        for p in Piano88Geometry.blackPitches {
            let r = Piano88Geometry.rect(forPitch: p, width: w, height: h)
            let keyR = CGRect(x: r.minX, y: 1, width: r.width, height: r.height)
            let path = CGPath(roundedRect: keyR, cornerWidth: 2, cornerHeight: 2, transform: nil)
            if let col = pitchColor[p] {
                ctx.saveGState()
                ctx.setShadow(offset: .zero, blur: 9, color: col.withAlphaComponent(0.95).cgColor)
                ctx.setFillColor((col.blended(withFraction: 0.25, of: .white) ?? col).cgColor)
                ctx.addPath(path)
                ctx.fillPath()
                ctx.restoreGState()
            } else {
                ctx.setFillColor(NSColor(red: 0.08, green: 0.09, blue: 0.12, alpha: 1).cgColor)
                ctx.addPath(path)
                ctx.fillPath()
                ctx.setStrokeColor(NSColor(white: 0.28, alpha: 0.6).cgColor)
                ctx.setLineWidth(0.7)
                ctx.addPath(path)
                ctx.strokePath()
            }
            if let q = qwertyLabels[p - baseC] {
                (q as NSString).draw(at: NSPoint(x: keyR.midX - 3, y: keyR.maxY - 14), withAttributes: qBlackAttrs)
            }
        }
    }

    private func pitchAt(_ pt: NSPoint) -> UInt8? {
        let w = bounds.width
        let h = bounds.height
        for p in Piano88Geometry.blackPitches {
            if Piano88Geometry.rect(forPitch: p, width: w, height: h).contains(pt) {
                return UInt8(p)
            }
        }
        for p in Piano88Geometry.whitePitches {
            if Piano88Geometry.rect(forPitch: p, width: w, height: h).contains(pt) {
                return UInt8(p)
            }
        }
        return nil
    }

    override func mouseDown(with event: NSEvent) {
        let pt = convert(event.locationInWindow, from: nil)
        guard let p = pitchAt(pt) else { return }
        draggingPitch = p
        store.liveInputPitches.insert(p)
        audio.auditionNoteOn(track: store.selectedTrack, pitch: p, velocity: 100)
        needsDisplay = true
    }

    override func mouseDragged(with event: NSEvent) {
        let pt = convert(event.locationInWindow, from: nil)
        guard let p = pitchAt(pt), p != draggingPitch else { return }
        if let old = draggingPitch {
            audio.auditionNoteOff(track: store.selectedTrack, pitch: old)
            store.liveInputPitches.remove(old)
        }
        draggingPitch = p
        store.liveInputPitches.insert(p)
        audio.auditionNoteOn(track: store.selectedTrack, pitch: p, velocity: 100)
        needsDisplay = true
    }

    override func mouseUp(with event: NSEvent) {
        if let old = draggingPitch {
            audio.auditionNoteOff(track: store.selectedTrack, pitch: old)
            store.liveInputPitches.remove(old)
            draggingPitch = nil
            needsDisplay = true
        }
    }
}

// MARK: - Falling-Notes Cascade Visualizer (streams orchestral notes onto the 88-key keyboard)

final class FallingNotesView: NSView {
    let store: StudioStore
    let audio: StudioAudioEngine
    var lookaheadSeconds: Double = 2.8

    override var isFlipped: Bool { true }

    init(store: StudioStore, audio: StudioAudioEngine) {
        self.store = store
        self.audio = audio
        super.init(frame: .zero)
        wantsLayer = true
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) not supported") }

    override func draw(_ dirtyRect: NSRect) {
        guard let ctx = NSGraphicsContext.current?.cgContext else { return }
        let w = bounds.width
        let h = bounds.height

        // Deep gothic opera velvet gradient background
        let colors = [
            NSColor(red: 0.032, green: 0.034, blue: 0.048, alpha: 1).cgColor,
            NSColor(red: 0.055, green: 0.042, blue: 0.068, alpha: 1).cgColor,
        ] as CFArray
        if let grad = CGGradient(colorsSpace: CGColorSpaceCreateDeviceRGB(), colors: colors, locations: [0, 1]) {
            ctx.drawLinearGradient(grad, start: CGPoint(x: 0, y: 0), end: CGPoint(x: 0, y: h), options: [])
        }

        // Subtle vertical octave guide lines matching the 88-key keyboard directly below
        for p in Piano88Geometry.whitePitches where ((p % 12) + 12) % 12 == 0 {
            let r = Piano88Geometry.rect(forPitch: p, width: w, height: h)
            ctx.setFillColor(NSColor(white: 1, alpha: 0.07).cgColor)
            ctx.fill(CGRect(x: r.minX, y: 0, width: 1, height: h))
        }

        let tracks = store.tracks()
        let trackMap = Dictionary(uniqueKeysWithValues: tracks.map { ($0.index, $0) })
        let anySolo = tracks.contains(where: { $0.solo })
        let nowSample = audio.currentSample
        let lookSamples = Int64(lookaheadSeconds * 48000.0)
        let nowTick = audio.currentTick

        // Active movement title watermark in upper-left
        let markers = store.markers()
        if let mi = markers.lastIndex(where: { $0.tick <= nowTick }) {
            let m = markers[mi]
            let bpm = store.withLock { Double(ms_bpm_at_tick(store.handle, nowTick)) / 1000.0 }
            let title = "\(m.label)  ·  \(String(format: "%.0f", bpm)) BPM" as NSString
            title.draw(at: NSPoint(x: 20, y: 18), withAttributes: [
                .font: NSFont.systemFont(ofSize: 15, weight: .semibold),
                .foregroundColor: StudioPalette.accentGold.withAlphaComponent(0.90),
            ])
        }

        // Draw falling note bars in sample-accurate time!
        let allNotes = store.notes()
        for n in allNotes {
            guard let tr = trackMap[Int32(n.track)], !tr.mute else { continue }
            if anySolo && !tr.solo { continue }
            let effPitch = max(Piano88Geometry.minPitch, min(Piano88Geometry.maxPitch, Int(n.pitch) + Int(tr.transposePreview)))

            let s0 = store.withLock { ms_tick_to_sample(store.handle, n.startTick) }
            let s1 = store.withLock { ms_tick_to_sample(store.handle, n.startTick + n.durTick) }
            if s1 < nowSample || s0 > nowSample + lookSamples { continue }

            let yBottom = h - (CGFloat(s0 - nowSample) / CGFloat(lookSamples)) * h
            let yTop = h - (CGFloat(s1 - nowSample) / CGFloat(lookSamples)) * h
            let kr = Piano88Geometry.rect(forPitch: effPitch, width: w, height: h)
            let rect = CGRect(x: kr.minX + 1.5, y: yTop, width: max(3, kr.width - 3), height: max(4, yBottom - yTop))

            let col = StudioPalette.sectionColor(tr.section)
            let hitting = nowSample >= s0 && nowSample < s1
            let path = CGPath(roundedRect: rect, cornerWidth: 3.5, cornerHeight: 3.5, transform: nil)

            if hitting {
                ctx.saveGState()
                ctx.setShadow(offset: .zero, blur: 14, color: col.withAlphaComponent(0.95).cgColor)
                ctx.setFillColor((col.blended(withFraction: 0.42, of: .white) ?? col).cgColor)
                ctx.addPath(path)
                ctx.fillPath()
                ctx.restoreGState()
            } else {
                let velAlpha = 0.55 + 0.45 * (CGFloat(n.velocity) / 127.0)
                ctx.setFillColor(col.withAlphaComponent(velAlpha).cgColor)
                ctx.addPath(path)
                ctx.fillPath()
            }
            ctx.setStrokeColor(NSColor.white.withAlphaComponent(hitting ? 0.9 : 0.25).cgColor)
            ctx.setLineWidth(hitting ? 1.4 : 0.7)
            ctx.addPath(path)
            ctx.strokePath()
        }

        // Glowing strike line right above the 88-key keyboard
        ctx.setFillColor(StudioPalette.accentGold.withAlphaComponent(0.55).cgColor)
        ctx.fill(CGRect(x: 0, y: h - 2, width: w, height: 2))
    }
}
