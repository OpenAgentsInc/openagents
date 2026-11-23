// Firm reason: 6 (macOS interactive Piano Roll + Velocity lane + vertical live keyboard NSView).
import AppKit
import Foundation

final class PianoRollView: NSView {
    let store: StudioStore
    let audio: StudioAudioEngine

    // Layout metrics
    private let keyRailWidth: CGFloat = 62
    private let rulerHeight: CGFloat = 46
    private let velocityHeight: CGFloat = 78

    // Viewport state
    var pixelsPerBeat: CGFloat = 72.0         // horizontal zoom (960 ticks = 1 beat)
    var rowHeight: CGFloat = 15.0             // vertical zoom per MIDI pitch
    var scrollX: CGFloat = 0                  // left tick in pixels
    var topPitch: CGFloat = 86.0              // highest visible MIDI pitch (D6 by default)
    var followPlayhead: Bool = true
    var pencilMode: Bool = false              // false = Select/Edit (double-click creates), true = Pencil click creates

    private let minPitch = 21                 // A0
    private let maxPitch = 108                // C8

    // Drag / interaction state
    private enum DragKind {
        case none
        case scrubRuler
        case moveNotes(anchorSlot: Int32, startTick: Int64, startPitch: Int, originals: [(slot: Int32, tick: Int64, pitch: UInt8, vel: UInt8, dur: Int64)], duplicate: Bool)
        case resizeRight(slot: Int32, origStart: Int64, origDur: Int64, pitch: UInt8, vel: UInt8)
        case resizeLeft(slot: Int32, origStart: Int64, origDur: Int64, pitch: UInt8, vel: UInt8)
        case marquee(origin: NSPoint, current: NSPoint)
        case keyRail(pitch: UInt8)
        case velocityDrag
    }
    private var drag: DragKind = .none
    private var lastAuditionPitch: Int = -1
    private var lastCreatedDuration: Int64 = 960

    var onMidiFileDropped: ((URL) -> Void)?
    var onSelectionChanged: (() -> Void)?

    override var isFlipped: Bool { true }
    override var acceptsFirstResponder: Bool { true }

    init(store: StudioStore, audio: StudioAudioEngine) {
        self.store = store
        self.audio = audio
        super.init(frame: .zero)
        wantsLayer = true
        registerForDraggedTypes([.fileURL])
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) not supported") }

    // MARK: - Coordinate transforms

    private var gridRect: NSRect {
        let h = max(100, bounds.height - rulerHeight - velocityHeight)
        return NSRect(x: keyRailWidth, y: rulerHeight, width: max(100, bounds.width - keyRailWidth), height: h)
    }

    private var velocityRect: NSRect {
        NSRect(x: keyRailWidth, y: bounds.height - velocityHeight, width: max(100, bounds.width - keyRailWidth), height: velocityHeight)
    }

    func xForTick(_ tick: Int64) -> CGFloat {
        keyRailWidth + (CGFloat(tick) / 960.0) * pixelsPerBeat - scrollX
    }

    func tickForX(_ x: CGFloat) -> Int64 {
        let beats = (x - keyRailWidth + scrollX) / pixelsPerBeat
        return max(0, Int64(round(beats * 960.0)))
    }

    func snapTick(_ tick: Int64, bypass: Bool = false) -> Int64 {
        if bypass || store.snapTicks <= 1 { return max(0, tick) }
        let s = store.snapTicks
        return max(0, ((tick + s / 2) / s) * s)
    }

    func yForPitch(_ pitch: Int) -> CGFloat {
        rulerHeight + (topPitch - CGFloat(pitch)) * rowHeight
    }

    func pitchForY(_ y: CGFloat) -> Int {
        let p = Int(floor(topPitch - (y - rulerHeight) / rowHeight)) + 1
        return max(minPitch, min(maxPitch, p))
    }

    func scrollToTick(_ tick: Int64) {
        let target = max(0, (CGFloat(tick) / 960.0) * pixelsPerBeat - 80)
        scrollX = target
        needsDisplay = true
    }

    func ensurePlayheadVisible() {
        guard followPlayhead && audio.isPlaying else { return }
        let px = xForTick(audio.currentTick)
        let gr = gridRect
        if px > gr.maxX - 90 || px < gr.minX {
            scrollX = max(0, (CGFloat(audio.currentTick) / 960.0) * pixelsPerBeat - gr.width * 0.22)
        }
    }

    private func isBlackKey(_ pitch: Int) -> Bool {
        let m = ((pitch % 12) + 12) % 12
        return m == 1 || m == 3 || m == 6 || m == 8 || m == 10
    }

    // MARK: - Drawing (60 fps)

    override func draw(_ dirtyRect: NSRect) {
        guard let ctx = NSGraphicsContext.current?.cgContext else { return }
        ensurePlayheadVisible()

        let w = bounds.width
        let h = bounds.height
        let gr = gridRect
        let vr = velocityRect

        // Background
        ctx.setFillColor(StudioPalette.bgCanvas.cgColor)
        ctx.fill(bounds)

        let tracks = store.tracks()
        let trackMap = Dictionary(uniqueKeysWithValues: tracks.map { ($0.index, $0) })
        let allNotes = store.notes()
        let markers = store.markers()
        let activeByTrack = store.activePitchesByTrack()
        let playTick = audio.currentTick

        let viewTickLo = max(0, tickForX(gr.minX) - 960)
        let viewTickHi = tickForX(gr.maxX) + 960

        // 1. Piano roll horizontal pitch rows + dynamic per-bar Scale Highlighting
        ctx.saveGState()
        ctx.clip(to: gr)

        let pHi = min(maxPitch, Int(ceil(topPitch)) + 1)
        let pLo = max(minPitch, Int(floor(topPitch - gr.height / rowHeight)) - 1)

        // Base row zebra shading (white vs black keys)
        for p in pLo...pHi {
            let y = yForPitch(p)
            let black = isBlackKey(p)
            ctx.setFillColor(black
                ? NSColor(red: 0.036, green: 0.038, blue: 0.048, alpha: 1).cgColor
                : NSColor(red: 0.056, green: 0.059, blue: 0.074, alpha: 1).cgColor)
            ctx.fill(CGRect(x: gr.minX, y: y, width: gr.width, height: rowHeight))

            // Subtle scale-in-key shading per movement segment
            for mi in 0..<markers.count {
                let m = markers[mi]
                let segStart = m.tick
                let segEnd = (mi + 1 < markers.count) ? markers[mi + 1].tick : max(viewTickHi, segStart + 8 * 3840)
                if segEnd < viewTickLo || segStart > viewTickHi { continue }
                if ms_pitch_in_scale(Int32(p), Int32(m.tonic), m.minor) {
                    let sx0 = max(gr.minX, xForTick(segStart))
                    let sx1 = min(gr.maxX, xForTick(segEnd))
                    if sx1 > sx0 {
                        let tint = m.minor
                            ? NSColor(red: 0.65, green: 0.22, blue: 0.34, alpha: 0.055)
                            : NSColor(red: 0.25, green: 0.56, blue: 0.85, alpha: 0.055)
                        ctx.setFillColor(tint.cgColor)
                        ctx.fill(CGRect(x: sx0, y: y, width: sx1 - sx0, height: rowHeight))
                    }
                }
            }

            // Horizontal divider (stronger below C)
            let isC = ((p % 12) + 12) % 12 == 0
            ctx.setFillColor(isC
                ? NSColor(white: 1, alpha: 0.14).cgColor
                : NSColor(white: 1, alpha: 0.045).cgColor)
            ctx.fill(CGRect(x: gr.minX, y: y + rowHeight - 0.5, width: gr.width, height: isC ? 1.0 : 0.5))
        }

        // 2. Vertical bar / beat / subdivision grid lines
        let stepTicks: Int64 = pixelsPerBeat >= 90 ? 240 : (pixelsPerBeat >= 45 ? 480 : 960)
        let firstGrid = (viewTickLo / stepTicks) * stepTicks
        var tk = firstGrid
        while tk <= viewTickHi {
            let x = xForTick(tk)
            if x >= gr.minX && x <= gr.maxX {
                let isBar = (tk % 3840) == 0
                let isBeat = (tk % 960) == 0
                ctx.setFillColor(isBar
                    ? NSColor(white: 1, alpha: 0.19).cgColor
                    : (isBeat ? NSColor(white: 1, alpha: 0.09).cgColor : NSColor(white: 1, alpha: 0.035).cgColor))
                ctx.fill(CGRect(x: x, y: gr.minY, width: isBar ? 1.0 : 0.5, height: gr.height))
            }
            tk += stepTicks
        }

        // Movement boundary vertical gold guidelines
        for m in markers where m.tick > 0 {
            let x = xForTick(m.tick)
            if x >= gr.minX && x <= gr.maxX {
                ctx.setFillColor(StudioPalette.accentGold.withAlphaComponent(0.32).cgColor)
                ctx.fill(CGRect(x: x, y: gr.minY, width: 1.5, height: gr.height))
            }
        }

        // Loop region shading
        if store.loopEnabled && store.loopEndTick > store.loopStartTick {
            let lx0 = max(gr.minX, xForTick(store.loopStartTick))
            let lx1 = min(gr.maxX, xForTick(store.loopEndTick))
            if lx1 > lx0 {
                ctx.setFillColor(StudioPalette.accentGold.withAlphaComponent(0.05).cgColor)
                ctx.fill(CGRect(x: lx0, y: gr.minY, width: lx1 - lx0, height: gr.height))
            }
        }

        // 3. Ghost Notes (other visible tracks behind selected track)
        for n in allNotes {
            if Int32(n.track) == store.selectedTrack { continue }
            guard let tr = trackMap[Int32(n.track)], tr.ghostVisible else { continue }
            if n.startTick + n.durTick < viewTickLo || n.startTick > viewTickHi { continue }
            let effPitch = max(0, min(127, Int(n.pitch) + Int(tr.transposePreview)))
            if effPitch < pLo || effPitch > pHi { continue }
            let x0 = xForTick(n.startTick)
            let x1 = xForTick(n.startTick + n.durTick)
            let y = yForPitch(effPitch)
            let rect = CGRect(x: x0, y: y + 1.5, width: max(3, x1 - x0 - 1), height: max(3, rowHeight - 3))
            let isSounding = audio.isPlaying && playTick >= n.startTick && playTick < n.startTick + n.durTick && !tr.mute
            let baseCol = StudioPalette.sectionColor(tr.section)
            let fillCol = baseCol.withAlphaComponent(isSounding ? 0.58 : 0.16)
            ctx.setFillColor(fillCol.cgColor)
            let path = CGPath(roundedRect: rect, cornerWidth: 2.5, cornerHeight: 2.5, transform: nil)
            ctx.addPath(path)
            ctx.fillPath()
            if isSounding {
                ctx.setStrokeColor(NSColor.white.withAlphaComponent(0.75).cgColor)
                ctx.setLineWidth(1.0)
                ctx.addPath(path)
                ctx.strokePath()
            }
        }

        // 4. Active Track Notes (editable, luminous, pitch labels when zoomed in)
        let activeTrack = trackMap[store.selectedTrack]
        let activeColor = StudioPalette.sectionColor(activeTrack?.section ?? 0)
        let activeShift = Int(activeTrack?.transposePreview ?? 0)
        let noteAttrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.monospacedSystemFont(ofSize: 9.5, weight: .semibold),
            .foregroundColor: NSColor(white: 0.05, alpha: 0.92),
        ]

        for n in allNotes where Int32(n.track) == store.selectedTrack {
            if n.startTick + n.durTick < viewTickLo || n.startTick > viewTickHi { continue }
            let effPitch = max(0, min(127, Int(n.pitch) + activeShift))
            if effPitch < pLo || effPitch > pHi { continue }
            let x0 = xForTick(n.startTick)
            let x1 = xForTick(n.startTick + n.durTick)
            let y = yForPitch(effPitch)
            let nw = max(4, x1 - x0 - 1)
            let rect = CGRect(x: x0, y: y + 1, width: nw, height: max(4, rowHeight - 2))
            let selected = store.selectedNoteSlots.contains(n.slot)
            let sounding = audio.isPlaying && playTick >= n.startTick && playTick < n.startTick + n.durTick

            let velAlpha = 0.45 + 0.55 * (CGFloat(n.velocity) / 127.0)
            let fill = sounding
                ? activeColor.blended(withFraction: 0.48, of: .white) ?? activeColor
                : (selected ? activeColor.blended(withFraction: 0.25, of: .white) ?? activeColor : activeColor.withAlphaComponent(velAlpha))

            let path = CGPath(roundedRect: rect, cornerWidth: 3, cornerHeight: 3, transform: nil)
            if sounding {
                ctx.saveGState()
                ctx.setShadow(offset: .zero, blur: 10, color: activeColor.withAlphaComponent(0.9).cgColor)
                ctx.setFillColor(fill.cgColor)
                ctx.addPath(path)
                ctx.fillPath()
                ctx.restoreGState()
            } else {
                ctx.setFillColor(fill.cgColor)
                ctx.addPath(path)
                ctx.fillPath()
            }

            // Border
            ctx.setStrokeColor(selected
                ? NSColor.white.cgColor
                : (sounding ? NSColor.white.withAlphaComponent(0.9).cgColor : NSColor.black.withAlphaComponent(0.45).cgColor))
            ctx.setLineWidth(selected ? 1.6 : (sounding ? 1.2 : 0.8))
            ctx.addPath(path)
            ctx.strokePath()

            // Pitch label inside wider notes
            if nw >= 26 && rowHeight >= 12 {
                let lbl = StudioPalette.pitchName(effPitch) as NSString
                lbl.draw(at: NSPoint(x: rect.minX + 4, y: rect.minY + 1), withAttributes: noteAttrs)
            }
        }

        // Marquee selection box
        if case let .marquee(origin, current) = drag {
            let mr = CGRect(
                x: min(origin.x, current.x), y: min(origin.y, current.y),
                width: abs(current.x - origin.x), height: abs(current.y - origin.y)
            )
            ctx.setFillColor(StudioPalette.accentGold.withAlphaComponent(0.14).cgColor)
            ctx.fill(mr)
            ctx.setStrokeColor(StudioPalette.accentGold.withAlphaComponent(0.85).cgColor)
            ctx.setLineWidth(1)
            ctx.stroke(mr)
        }

        // Playhead vertical line in Piano Roll
        let phX = xForTick(playTick)
        if phX >= gr.minX && phX <= gr.maxX {
            ctx.setFillColor(StudioPalette.playhead.cgColor)
            ctx.fill(CGRect(x: phX - 0.75, y: gr.minY, width: 1.5, height: gr.height))
        }
        ctx.restoreGState()

        // 5. Left Vertical Interactive 88-Key Keyboard Rail
        drawKeyRail(ctx: ctx, pLo: pLo, pHi: pHi, activeByTrack: activeByTrack, trackMap: trackMap)

        // 6. Top Bar Ruler & Movement Banners
        drawRuler(ctx: ctx, markers: markers, viewTickLo: viewTickLo, viewTickHi: viewTickHi, playTick: playTick)

        // 7. Bottom Velocity Lane
        drawVelocityLane(ctx: ctx, rect: vr, notes: allNotes, activeColor: activeColor, viewTickLo: viewTickLo, viewTickHi: viewTickHi, playTick: playTick)

        // Outer borders
        ctx.setFillColor(StudioPalette.border.cgColor)
        ctx.fill(CGRect(x: keyRailWidth - 1, y: 0, width: 1, height: h))
        ctx.fill(CGRect(x: 0, y: rulerHeight - 1, width: w, height: 1))
        ctx.fill(CGRect(x: 0, y: vr.minY, width: w, height: 1))
    }

    private func drawKeyRail(ctx: CGContext, pLo: Int, pHi: Int, activeByTrack: [UInt8: [UInt8]], trackMap: [Int32: TrackViewItem]) {
        let railRect = CGRect(x: 0, y: rulerHeight, width: keyRailWidth, height: gridRect.height)
        ctx.saveGState()
        ctx.clip(to: railRect)

        // Build pitch -> live color map
        var litPitchColor: [Int: NSColor] = [:]
        for (trIdx, pitches) in activeByTrack {
            let sec = trackMap[Int32(trIdx)]?.section ?? 0
            let col = StudioPalette.sectionColor(sec)
            for p in pitches { litPitchColor[Int(p)] = col }
        }
        for p in store.liveInputPitches {
            litPitchColor[Int(p)] = StudioPalette.accentGold
        }

        let cAttrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.monospacedSystemFont(ofSize: 9, weight: .medium),
            .foregroundColor: NSColor(white: 0.40, alpha: 1),
        ]

        for p in pLo...pHi {
            let y = yForPitch(p)
            let black = isBlackKey(p)
            let lit = litPitchColor[p]
            let row = CGRect(x: 0, y: y, width: keyRailWidth - 1, height: rowHeight)
            if let col = lit {
                ctx.setFillColor(col.cgColor)
                ctx.fill(row)
            } else if black {
                ctx.setFillColor(NSColor(red: 0.10, green: 0.11, blue: 0.13, alpha: 1).cgColor)
                ctx.fill(row)
                ctx.setFillColor(NSColor(red: 0.16, green: 0.17, blue: 0.21, alpha: 1).cgColor)
                ctx.fill(CGRect(x: 0, y: y + 1, width: keyRailWidth * 0.64, height: max(2, rowHeight - 2)))
            } else {
                ctx.setFillColor(NSColor(red: 0.86, green: 0.87, blue: 0.90, alpha: 1).cgColor)
                ctx.fill(row)
            }
            ctx.setFillColor(NSColor(white: 0.15, alpha: 0.45).cgColor)
            ctx.fill(CGRect(x: 0, y: y + rowHeight - 0.5, width: keyRailWidth - 1, height: 0.5))

            if ((p % 12) + 12) % 12 == 0 {
                let lbl = "C\((p / 12) - 1)" as NSString
                lbl.draw(at: NSPoint(x: keyRailWidth - 22, y: y + max(0, (rowHeight - 11) / 2)), withAttributes: cAttrs)
            }
        }
        ctx.restoreGState()
    }

    private func drawRuler(ctx: CGContext, markers: [MarkerViewItem], viewTickLo: Int64, viewTickHi: Int64, playTick: Int64) {
        let rRect = CGRect(x: 0, y: 0, width: bounds.width, height: rulerHeight)
        ctx.setFillColor(StudioPalette.bgHeader.cgColor)
        ctx.fill(rRect)

        // Corner label
        let cornerAttrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.monospacedSystemFont(ofSize: 9.5, weight: .semibold),
            .foregroundColor: StudioPalette.textMuted,
        ]
        ("PITCH" as NSString).draw(at: NSPoint(x: 12, y: 16), withAttributes: cornerAttrs)

        ctx.saveGState()
        ctx.clip(to: CGRect(x: keyRailWidth, y: 0, width: bounds.width - keyRailWidth, height: rulerHeight))

        // Loop bar highlight
        if store.loopEnabled && store.loopEndTick > store.loopStartTick {
            let lx0 = xForTick(store.loopStartTick)
            let lx1 = xForTick(store.loopEndTick)
            ctx.setFillColor(StudioPalette.accentGold.withAlphaComponent(0.24).cgColor)
            ctx.fill(CGRect(x: lx0, y: 0, width: lx1 - lx0, height: 6))
        }

        // Movement banners on upper half of ruler
        let mAttrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: 10, weight: .semibold),
            .foregroundColor: StudioPalette.accentGold,
        ]
        for m in markers {
            let x = xForTick(m.tick)
            ctx.setFillColor(StudioPalette.accentGold.withAlphaComponent(0.18).cgColor)
            ctx.fill(CGRect(x: x, y: 4, width: 175, height: 17))
            ctx.setFillColor(StudioPalette.accentGold.cgColor)
            ctx.fill(CGRect(x: x, y: 4, width: 2.5, height: 17))
            (m.label as NSString).draw(at: NSPoint(x: x + 6, y: 6), withAttributes: mAttrs)
        }

        // Bar numbers on lower half of ruler
        let barAttrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.monospacedSystemFont(ofSize: 10, weight: .medium),
            .foregroundColor: StudioPalette.textMuted,
        ]
        let firstBar = max(0, viewTickLo / 3840)
        let lastBar = (viewTickHi / 3840) + 1
        for b in firstBar...lastBar {
            let x = xForTick(b * 3840)
            ctx.setFillColor(NSColor(white: 1, alpha: 0.22).cgColor)
            ctx.fill(CGRect(x: x, y: 24, width: 1, height: 22))
            ("\(b + 1)" as NSString).draw(at: NSPoint(x: x + 4, y: 27), withAttributes: barAttrs)
        }

        // Playhead triangle
        let phX = xForTick(playTick)
        ctx.setFillColor(StudioPalette.playhead.cgColor)
        ctx.beginPath()
        ctx.move(to: CGPoint(x: phX - 6, y: rulerHeight - 10))
        ctx.addLine(to: CGPoint(x: phX + 6, y: rulerHeight - 10))
        ctx.addLine(to: CGPoint(x: phX, y: rulerHeight))
        ctx.closePath()
        ctx.fillPath()
        ctx.restoreGState()
    }

    private func drawVelocityLane(ctx: CGContext, rect: CGRect, notes: [NoteViewItem], activeColor: NSColor, viewTickLo: Int64, viewTickHi: Int64, playTick: Int64) {
        ctx.setFillColor(StudioPalette.bgPanel.cgColor)
        ctx.fill(CGRect(x: 0, y: rect.minY, width: bounds.width, height: rect.height))

        let lblAttrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.monospacedSystemFont(ofSize: 9.5, weight: .semibold),
            .foregroundColor: StudioPalette.textMuted,
        ]
        ("VEL" as NSString).draw(at: NSPoint(x: 16, y: rect.minY + 10), withAttributes: lblAttrs)

        ctx.saveGState()
        ctx.clip(to: rect)
        // Midline (vel 64)
        ctx.setFillColor(NSColor(white: 1, alpha: 0.06).cgColor)
        ctx.fill(CGRect(x: rect.minX, y: rect.minY + rect.height * 0.5, width: rect.width, height: 0.5))

        for n in notes where Int32(n.track) == store.selectedTrack {
            if n.startTick < viewTickLo || n.startTick > viewTickHi { continue }
            let x = xForTick(n.startTick)
            let frac = CGFloat(n.velocity) / 127.0
            let barH = max(4, frac * (rect.height - 14))
            let y = rect.maxY - barH - 4
            let selected = store.selectedNoteSlots.contains(n.slot)
            let col = selected ? NSColor.white : activeColor.withAlphaComponent(0.75)
            ctx.setFillColor(col.cgColor)
            ctx.fill(CGRect(x: x, y: y, width: 4, height: barH))
            ctx.fillEllipse(in: CGRect(x: x - 1.5, y: y - 2.5, width: 7, height: 7))
        }

        let phX = xForTick(playTick)
        ctx.setFillColor(StudioPalette.playhead.withAlphaComponent(0.6).cgColor)
        ctx.fill(CGRect(x: phX - 0.5, y: rect.minY, width: 1, height: rect.height))
        ctx.restoreGState()
    }

    // MARK: - Hit Testing & Mouse Editing

    private func hitNote(at pt: NSPoint) -> (note: NoteViewItem, edge: Int)? {
        // edge: 0 = body, -1 = left edge, +1 = right edge
        let track = store.tracks().first(where: { $0.index == store.selectedTrack })
        let shift = Int(track?.transposePreview ?? 0)
        for n in store.notes().reversed() where Int32(n.track) == store.selectedTrack {
            let effPitch = max(0, min(127, Int(n.pitch) + shift))
            let x0 = xForTick(n.startTick)
            let x1 = max(x0 + 6, xForTick(n.startTick + n.durTick))
            let y0 = yForPitch(effPitch)
            let r = CGRect(x: x0 - 2, y: y0, width: (x1 - x0) + 4, height: rowHeight)
            if r.contains(pt) {
                if pt.x >= x1 - 7 && (x1 - x0) >= 12 { return (n, 1) }
                if pt.x <= x0 + 6 && (x1 - x0) >= 16 { return (n, -1) }
                return (n, 0)
            }
        }
        return nil
    }

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
        let pt = convert(event.locationInWindow, from: nil)
        let bypassSnap = event.modifierFlags.contains(.command)

        // 1. Ruler scrub
        if pt.y < rulerHeight && pt.x >= keyRailWidth {
            drag = .scrubRuler
            audio.seek(tick: snapTick(tickForX(pt.x), bypass: bypassSnap))
            needsDisplay = true
            return
        }

        // 2. Left key rail click -> audition pitch
        if pt.x < keyRailWidth && pt.y >= rulerHeight && pt.y < velocityRect.minY {
            let p = UInt8(pitchForY(pt.y))
            drag = .keyRail(pitch: p)
            store.liveInputPitches.insert(p)
            audio.auditionNoteOn(track: store.selectedTrack, pitch: p, velocity: 96)
            needsDisplay = true
            return
        }

        // 3. Velocity lane edit
        if velocityRect.contains(pt) {
            drag = .velocityDrag
            applyVelocityDrag(at: pt)
            return
        }

        // 4. Piano roll note interaction
        if gridRect.contains(pt) {
            if let hit = hitNote(at: pt) {
                if hit.edge == 1 {
                    store.selectedNoteSlots = [hit.note.slot]
                    drag = .resizeRight(slot: hit.note.slot, origStart: hit.note.startTick, origDur: hit.note.durTick, pitch: hit.note.pitch, vel: hit.note.velocity)
                    onSelectionChanged?()
                    needsDisplay = true
                    return
                }
                if hit.edge == -1 {
                    store.selectedNoteSlots = [hit.note.slot]
                    drag = .resizeLeft(slot: hit.note.slot, origStart: hit.note.startTick, origDur: hit.note.durTick, pitch: hit.note.pitch, vel: hit.note.velocity)
                    onSelectionChanged?()
                    needsDisplay = true
                    return
                }
                if !store.selectedNoteSlots.contains(hit.note.slot) {
                    if event.modifierFlags.contains(.shift) {
                        store.selectedNoteSlots.insert(hit.note.slot)
                    } else {
                        store.selectedNoteSlots = [hit.note.slot]
                    }
                }
                let duplicate = event.modifierFlags.contains(.option)
                let all = store.notes()
                var origs: [(slot: Int32, tick: Int64, pitch: UInt8, vel: UInt8, dur: Int64)] = []
                for n in all where store.selectedNoteSlots.contains(n.slot) {
                    if duplicate {
                        let newSlot = store.withLock {
                            ms_add_note(store.handle, n.track, n.pitch, n.velocity, n.startTick, n.durTick)
                        }
                        if newSlot >= 0 {
                            origs.append((newSlot, n.startTick, n.pitch, n.velocity, n.durTick))
                        }
                    } else {
                        origs.append((n.slot, n.startTick, n.pitch, n.velocity, n.durTick))
                    }
                }
                if duplicate {
                    store.selectedNoteSlots = Set(origs.map { $0.slot })
                }
                let p = pitchForY(pt.y)
                lastAuditionPitch = p
                audio.auditionNoteOn(track: store.selectedTrack, pitch: hit.note.pitch, velocity: hit.note.velocity)
                drag = .moveNotes(anchorSlot: hit.note.slot, startTick: tickForX(pt.x), startPitch: p, originals: origs, duplicate: duplicate)
                onSelectionChanged?()
                needsDisplay = true
                return
            }

            // Empty grid: Pencil mode OR double-click creates a new note!
            if pencilMode || event.clickCount >= 2 {
                let tk = snapTick(tickForX(pt.x), bypass: bypassSnap)
                let tr = store.tracks().first(where: { $0.index == store.selectedTrack })
                let shift = Int(tr?.transposePreview ?? 0)
                let p = UInt8(max(0, min(127, pitchForY(pt.y) - shift)))
                let slot = store.withLock {
                    ms_add_note(store.handle, UInt8(store.selectedTrack), p, 96, tk, lastCreatedDuration)
                }
                if slot >= 0 {
                    store.selectedNoteSlots = [slot]
                    audio.auditionNoteOn(track: store.selectedTrack, pitch: UInt8(max(0, min(127, Int(p) + shift))), velocity: 96)
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.22) { [weak self] in
                        guard let self = self else { return }
                        self.audio.auditionNoteOff(track: self.store.selectedTrack, pitch: UInt8(max(0, min(127, Int(p) + shift))))
                    }
                    drag = .resizeRight(slot: slot, origStart: tk, origDur: lastCreatedDuration, pitch: p, vel: 96)
                    store.onChange?()
                    onSelectionChanged?()
                    needsDisplay = true
                }
                return
            }

            // Otherwise start marquee selection
            if !event.modifierFlags.contains(.shift) {
                store.selectedNoteSlots.removeAll()
                onSelectionChanged?()
            }
            drag = .marquee(origin: pt, current: pt)
            needsDisplay = true
        }
    }

    override func mouseDragged(with event: NSEvent) {
        let pt = convert(event.locationInWindow, from: nil)
        let bypassSnap = event.modifierFlags.contains(.command)

        switch drag {
        case .none:
            break
        case .scrubRuler:
            audio.seek(tick: snapTick(tickForX(pt.x), bypass: bypassSnap))
            needsDisplay = true
        case .keyRail(let oldP):
            let newP = UInt8(pitchForY(pt.y))
            if newP != oldP {
                audio.auditionNoteOff(track: store.selectedTrack, pitch: oldP)
                store.liveInputPitches.remove(oldP)
                store.liveInputPitches.insert(newP)
                audio.auditionNoteOn(track: store.selectedTrack, pitch: newP, velocity: 96)
                drag = .keyRail(pitch: newP)
                needsDisplay = true
            }
        case .velocityDrag:
            applyVelocityDrag(at: pt)
        case .moveNotes(_, let startTick, let startPitch, let originals, _):
            let rawDt = tickForX(pt.x) - startTick
            let dt = bypassSnap ? rawDt : (rawDt / max(1, store.snapTicks)) * store.snapTicks
            let dp = pitchForY(pt.y) - startPitch
            let shiftLock = event.modifierFlags.contains(.shift)
            let useDt = (shiftLock && abs(dp) > 1) ? 0 : dt
            let useDp = (shiftLock && abs(dt) > 240) ? 0 : dp
            for o in originals {
                let nt = max(0, o.tick + useDt)
                let np = UInt8(max(0, min(127, Int(o.pitch) + useDp)))
                _ = store.withLock { ms_update_note(store.handle, o.slot, np, o.vel, nt, o.dur) }
            }
            let nowPitch = max(0, min(127, startPitch + useDp))
            if nowPitch != lastAuditionPitch {
                if lastAuditionPitch >= 0 {
                    audio.auditionNoteOff(track: store.selectedTrack, pitch: UInt8(lastAuditionPitch))
                }
                audio.auditionNoteOn(track: store.selectedTrack, pitch: UInt8(nowPitch), velocity: 96)
                lastAuditionPitch = nowPitch
            }
            needsDisplay = true
        case .resizeRight(let slot, let origStart, _, let pitch, let vel):
            let endTick = snapTick(tickForX(pt.x), bypass: bypassSnap)
            let newDur = max(max(60, store.snapTicks / 2), endTick - origStart)
            lastCreatedDuration = newDur
            _ = store.withLock { ms_update_note(store.handle, slot, pitch, vel, origStart, newDur) }
            needsDisplay = true
        case .resizeLeft(let slot, let origStart, let origDur, let pitch, let vel):
            let origEnd = origStart + origDur
            let newStart = min(origEnd - 60, snapTick(tickForX(pt.x), bypass: bypassSnap))
            let newDur = max(60, origEnd - newStart)
            _ = store.withLock { ms_update_note(store.handle, slot, pitch, vel, newStart, newDur) }
            needsDisplay = true
        case .marquee(let origin, _):
            drag = .marquee(origin: origin, current: pt)
            let mr = CGRect(
                x: min(origin.x, pt.x), y: min(origin.y, pt.y),
                width: abs(pt.x - origin.x), height: abs(pt.y - origin.y)
            )
            var sel: Set<Int32> = []
            for n in store.notes() where Int32(n.track) == store.selectedTrack {
                let r = CGRect(x: xForTick(n.startTick), y: yForPitch(Int(n.pitch)), width: max(4, xForTick(n.startTick + n.durTick) - xForTick(n.startTick)), height: rowHeight)
                if r.intersects(mr) { sel.insert(n.slot) }
            }
            store.selectedNoteSlots = sel
            needsDisplay = true
        }
    }

    override func mouseUp(with event: NSEvent) {
        switch drag {
        case .keyRail(let p):
            audio.auditionNoteOff(track: store.selectedTrack, pitch: p)
            store.liveInputPitches.remove(p)
        case .moveNotes:
            if lastAuditionPitch >= 0 {
                audio.auditionNoteOff(track: store.selectedTrack, pitch: UInt8(lastAuditionPitch))
                lastAuditionPitch = -1
            }
            store.onChange?()
        case .resizeRight, .resizeLeft, .velocityDrag:
            store.onChange?()
        case .marquee:
            onSelectionChanged?()
        default:
            break
        }
        drag = .none
        needsDisplay = true
    }

    private func applyVelocityDrag(at pt: NSPoint) {
        let vr = velocityRect
        let frac = max(0.05, min(1.0, (vr.maxY - 4 - pt.y) / (vr.height - 14)))
        let vel = UInt8(max(8, min(127, Int(round(frac * 127.0)))))
        let hitTick = tickForX(pt.x)
        let tol = max(Int64(120), Int64(10.0 / pixelsPerBeat * 960.0))
        for n in store.notes() where Int32(n.track) == store.selectedTrack {
            if abs(n.startTick - hitTick) <= tol {
                _ = store.withLock { ms_update_note(store.handle, n.slot, n.pitch, vel, n.startTick, n.durTick) }
            }
        }
        needsDisplay = true
    }

    override func scrollWheel(with event: NSEvent) {
        if event.modifierFlags.contains(.command) {
            let factor: CGFloat = event.scrollingDeltaY > 0 ? 1.12 : 0.89
            pixelsPerBeat = max(18, min(260, pixelsPerBeat * factor))
        } else if event.modifierFlags.contains(.option) {
            let factor: CGFloat = event.scrollingDeltaY > 0 ? 1.10 : 0.91
            rowHeight = max(8, min(32, rowHeight * factor))
        } else {
            scrollX = max(0, scrollX - event.scrollingDeltaX * 2.0)
            topPitch = max(48, min(CGFloat(maxPitch), topPitch + event.scrollingDeltaY * 0.14))
        }
        needsDisplay = true
    }

    override func keyDown(with event: NSEvent) {
        let mods = event.modifierFlags
        // Arrow key note nudging / transposing when notes selected
        if !store.selectedNoteSlots.isEmpty {
            let step: Int64 = store.snapTicks
            if event.keyCode == 126 || event.keyCode == 125 { // Up / Down
                let dp = (event.keyCode == 126 ? 1 : -1) * (mods.contains(.shift) ? 12 : 1)
                for n in store.notes() where store.selectedNoteSlots.contains(n.slot) {
                    let np = UInt8(max(0, min(127, Int(n.pitch) + dp)))
                    _ = store.withLock { ms_update_note(store.handle, n.slot, np, n.velocity, n.startTick, n.durTick) }
                }
                store.onChange?()
                needsDisplay = true
                return
            }
            if event.keyCode == 123 || event.keyCode == 124 { // Left / Right
                let dt = (event.keyCode == 124 ? step : -step)
                for n in store.notes() where store.selectedNoteSlots.contains(n.slot) {
                    let nt = max(0, n.startTick + dt)
                    _ = store.withLock { ms_update_note(store.handle, n.slot, n.pitch, n.velocity, nt, n.durTick) }
                }
                store.onChange?()
                needsDisplay = true
                return
            }
            if event.keyCode == 51 || event.keyCode == 117 { // Backspace / Delete
                for slot in store.selectedNoteSlots {
                    _ = store.withLock { ms_remove_note(store.handle, slot) }
                }
                store.selectedNoteSlots.removeAll()
                store.onChange?()
                onSelectionChanged?()
                needsDisplay = true
                return
            }
        }
        super.keyDown(with: event)
    }

    // MARK: - Drag & Drop .mid files onto Piano Roll

    override func draggingEntered(_ sender: NSDraggingInfo) -> NSDragOperation {
        if droppedMidiURL(from: sender) != nil { return .copy }
        return []
    }

    override func performDragOperation(_ sender: NSDraggingInfo) -> Bool {
        guard let url = droppedMidiURL(from: sender) else { return false }
        onMidiFileDropped?(url)
        return true
    }

    private func droppedMidiURL(from info: NSDraggingInfo) -> URL? {
        guard let urls = info.draggingPasteboard.readObjects(forClasses: [NSURL.self], options: nil) as? [URL] else { return nil }
        return urls.first { ["mid", "midi", "smf"].contains($0.pathExtension.lowercased()) }
    }
}
