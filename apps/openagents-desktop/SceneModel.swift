// Firm reason: 6 (AppKit/Metal 3D scene viewport over CAMetalLayer for PLE benchmark scenes).
// Reusable 3D PLE Scene visualizer (GitHub #13792, Deliverable 3):
// - PleSceneMetalView: CAMetalLayer-backed Metal renderer for ple-scene.v1.json boxes, cylinders,
//   and clash markers with directional shading, depth buffer, semantic layer colors, orbit/pan/zoom,
//   camera presets (Iso, Plan, Overhead, Under-Floor), and raycast picking.
// - PleScenePanelView: Reusable container with HUD overlays (run summary, camera presets,
//   layer visibility toggles, and selected-element inspector).
import AppKit
import Metal
import ImageIO
import UniformTypeIdentifiers
import QuartzCore
import simd

// Split from SceneView.swift (#13848): ple-scene.v1 data model, diff, and loader.

// MARK: - ple-scene.v1 Data Model

enum PleSemanticLayer: String, CaseIterable, Hashable {
    case architecture
    case racks
    case crah
    case pdu
    case sprinklers
    case tray
    case busway
    case chw_pipe
    case clashes

    var displayName: String {
        switch self {
        case .architecture: return "Arch"
        case .racks: return "Racks"
        case .crah: return "CRAH"
        case .pdu: return "PDU"
        case .sprinklers: return "Sprinklers"
        case .tray: return "Tray"
        case .busway: return "Busway"
        case .chw_pipe: return "CHW Pipe"
        case .clashes: return "Clashes"
        }
    }

    var rgba: SIMD4<Float> {
        switch self {
        case .architecture: return SIMD4<Float>(0.58, 0.62, 0.68, 0.26)
        case .racks:        return SIMD4<Float>(0.22, 0.26, 0.34, 1.00)
        case .crah:         return SIMD4<Float>(0.12, 0.68, 0.64, 1.00)
        case .pdu:          return SIMD4<Float>(0.94, 0.62, 0.14, 1.00)
        case .sprinklers:   return SIMD4<Float>(0.48, 0.78, 0.98, 1.00)
        case .tray:         return SIMD4<Float>(0.24, 0.76, 0.42, 1.00)
        case .busway:       return SIMD4<Float>(0.98, 0.78, 0.16, 1.00)
        case .chw_pipe:     return SIMD4<Float>(0.12, 0.60, 0.95, 1.00)
        case .clashes:      return SIMD4<Float>(0.98, 0.14, 0.18, 1.00)
        }
    }

    var nsColor: NSColor {
        let c = rgba
        return NSColor(calibratedRed: CGFloat(c.x), green: CGFloat(c.y), blue: CGFloat(c.z), alpha: 1.0)
    }

    var isTranslucent: Bool {
        self == .architecture
    }

    static func classify(layer: String, id: String, ifcClass: String?) -> PleSemanticLayer {
        let l = layer.lowercased()
        let lid = id.lowercased()
        let cls = (ifcClass ?? "").lowercased()
        if let exact = PleSemanticLayer(rawValue: l) { return exact }
        switch l {
        case "clash", "clashes":
            return .clashes
        case "cooling":
            if lid.contains("pipe") || lid.contains("chw") || cls.contains("pipe") {
                return .chw_pipe
            }
            return .crah
        case "power":
            if lid.contains("busway") || cls.contains("cablecarrier") || cls.contains("busbar") {
                return .busway
            }
            return .pdu
        case "fire":
            return .sprinklers
        case "comms", "telecom", "cable_tray":
            return .tray
        default:
            if lid.contains("clash") { return .clashes }
            if lid.contains("rack") { return .racks }
            if lid.contains("crah") { return .crah }
            if lid.contains("pdu") { return .pdu }
            if lid.contains("sprink") { return .sprinklers }
            if lid.contains("tray") { return .tray }
            if lid.contains("busway") { return .busway }
            if lid.contains("pipe") || lid.contains("chw") { return .chw_pipe }
            return .architecture
        }
    }
}

enum PleJSONValue: Decodable, CustomStringConvertible {
    case string(String)
    case number(Double)
    case bool(Bool)
    case array([PleJSONValue])
    case object([String: PleJSONValue])
    case null

    init(from decoder: Decoder) throws {
        let c = try decoder.singleValueContainer()
        if c.decodeNil() { self = .null }
        else if let b = try? c.decode(Bool.self) { self = .bool(b) }
        else if let n = try? c.decode(Double.self) { self = .number(n) }
        else if let s = try? c.decode(String.self) { self = .string(s) }
        else if let a = try? c.decode([PleJSONValue].self) { self = .array(a) }
        else if let o = try? c.decode([String: PleJSONValue].self) { self = .object(o) }
        else { self = .null }
    }

    var description: String {
        switch self {
        case .string(let s): return s
        case .number(let n):
            if n.rounded() == n && abs(n) < 1e9 { return String(Int(n)) }
            return String(format: "%.2f", n)
        case .bool(let b): return b ? "true" : "false"
        case .array(let a): return "[" + a.map(\.description).joined(separator: ", ") + "]"
        case .object(let o): return o.map { "\($0.key)=\($0.value)" }.sorted().joined(separator: ", ")
        case .null: return "null"
        }
    }
}

struct PleSceneBounds: Decodable {
    let min: [Float]
    let max: [Float]

    var minVec: SIMD3<Float> {
        SIMD3<Float>(min.count > 0 ? min[0] : 0, min.count > 1 ? min[1] : 0, min.count > 2 ? min[2] : 0)
    }
    var maxVec: SIMD3<Float> {
        SIMD3<Float>(max.count > 0 ? max[0] : 10, max.count > 1 ? max[1] : 10, max.count > 2 ? max[2] : 5)
    }
    var center: SIMD3<Float> { (minVec + maxVec) * 0.5 }
    var extent: SIMD3<Float> { simd_max(maxVec - minVec, SIMD3<Float>(repeating: 0.1)) }
}

struct PleSceneRun: Decodable {
    let score: Double?
    let gates: [String: PleJSONValue]?
    let total_length_m: Double?
    let clash_count: Int?
    let passed_gates: Int?
    let total_gates: Int?

    var gateSummary: (passed: Int, total: Int)? {
        if let p = passed_gates, let t = total_gates { return (p, t) }
        guard let g = gates, !g.isEmpty else { return nil }
        var passed = 0
        for (_, v) in g {
            switch v {
            case .bool(let b): if b { passed += 1 }
            case .string(let s): if s.lowercased() == "pass" || s.lowercased() == "passed" || s.lowercased() == "true" { passed += 1 }
            case .number(let n): if n > 0 { passed += 1 }
            default: break
            }
        }
        return (passed, g.count)
    }
}

struct PleSceneElement: Decodable {
    let id: String
    let layer: String
    let ifc_class: String?
    let shape: String
    let center: [Float]
    let size: [Float]
    let rot_z: Float?
    var axis: String? = nil
    let meta: [String: PleJSONValue]?

    var centerVec: SIMD3<Float> {
        SIMD3<Float>(center.count > 0 ? center[0] : 0, center.count > 1 ? center[1] : 0, center.count > 2 ? center[2] : 0)
    }
    var sizeVec: SIMD3<Float> {
        SIMD3<Float>(max(0.02, size.count > 0 ? size[0] : 0.2),
                     max(0.02, size.count > 1 ? size[1] : 0.2),
                     max(0.02, size.count > 2 ? size[2] : 0.2))
    }
    var rotZ: Float { rot_z ?? 0 }
    var isCylinder: Bool { shape.lowercased() == "cylinder" }
    /// The local axis a cylinder's length runs along (ple-scene.v1 "axis": "x", "y", or "z", the
    /// default). size is its box in its own frame, so a pipe along x is [length, d, d].
    var axisIndex: Int {
        switch (axis ?? "z").lowercased() {
        case "x": return 0
        case "y": return 1
        default: return 2
        }
    }
    var semanticLayer: PleSemanticLayer {
        PleSemanticLayer.classify(layer: layer, id: id, ifcClass: ifc_class)
    }
}

struct PleSceneClash: Decodable {
    let a: String
    let b: String
    let point: [Float]
    let penetration_mm: Double?

    func asElement(index: Int) -> PleSceneElement {
        let d = Float(max(0.32, min(0.8, (penetration_mm ?? 50.0) / 100.0)))
        var meta: [String: PleJSONValue] = [
            "pair": .array([.string(a), .string(b)])
        ]
        if let pen = penetration_mm {
            meta["penetration_mm"] = .number(pen)
        }
        return PleSceneElement(
            id: "clash_\(index + 1)",
            layer: "clashes",
            ifc_class: "PleClashMarker",
            shape: "box",
            center: point,
            size: [d, d, d],
            rot_z: 0,
            axis: nil,
            meta: meta
        )
    }
}

struct PleScene: Decodable {
    let version: String
    let task: String?
    let units: String?
    let bounds: PleSceneBounds
    let run: PleSceneRun?
    let elements: [PleSceneElement]
    let clashes: [PleSceneClash]?

    var allElements: [PleSceneElement] {
        guard let clashes = clashes, !clashes.isEmpty else { return elements }
        var out = elements
        for (i, c) in clashes.enumerated() {
            out.append(c.asElement(index: i))
        }
        return out
    }

    var totalClashCount: Int {
        let top = clashes?.count ?? 0
        let elemClashes = elements.filter { $0.semanticLayer == .clashes }.count
        return max(top + elemClashes, run?.clash_count ?? 0)
    }

    /// Unified list of clashes from both top-level `clashes` and clash-layer elements.
    var resolvedClashes: [PleResolvedClash] {
        var out: [PleResolvedClash] = []
        if let top = clashes {
            for (i, c) in top.enumerated() {
                let pt = SIMD3<Float>(
                    c.point.count > 0 ? c.point[0] : 0,
                    c.point.count > 1 ? c.point[1] : 0,
                    c.point.count > 2 ? c.point[2] : 0
                )
                out.append(PleResolvedClash(
                    id: "clash_\(i + 1)",
                    elementId: "clash_\(i + 1)",
                    a: c.a,
                    b: c.b,
                    point: pt,
                    penetrationMM: c.penetration_mm,
                    severity: nil
                ))
            }
        }
        for elem in elements where elem.semanticLayer == .clashes {
            var aName = elem.id
            var bName = "clash"
            var penMM: Double? = nil
            var sev: String? = nil
            if let meta = elem.meta {
                if case .array(let arr)? = meta["pair"], arr.count >= 2 {
                    aName = arr[0].description
                    bName = arr[1].description
                } else {
                    if let av = meta["a"] { aName = av.description }
                    if let bv = meta["b"] { bName = bv.description }
                }
                if case .number(let n)? = meta["penetration_mm"] {
                    penMM = n
                }
                if case .string(let s)? = meta["severity"] {
                    sev = s
                }
            }
            out.append(PleResolvedClash(
                id: elem.id,
                elementId: elem.id,
                a: aName,
                b: bName,
                point: elem.centerVec,
                penetrationMM: penMM,
                severity: sev
            ))
        }
        return out
    }

    static func load(from url: URL) throws -> PleScene {
        let data = try Data(contentsOf: url)
        return try JSONDecoder().decode(PleScene.self, from: data)
    }
}

/// Normalized clash item used by the Stage 3 Clash Inspector and 3D camera focus.
struct PleResolvedClash: Equatable {
    let id: String
    let elementId: String
    let a: String
    let b: String
    let point: SIMD3<Float>
    let penetrationMM: Double?
    let severity: String?

    var pairLabel: String {
        "\(a) ↔ \(b)"
    }

    var detailLabel: String {
        var parts: [String] = []
        if let pen = penetrationMM {
            parts.append(String(format: "%.1f mm", pen))
        } else if let sev = severity, !sev.isEmpty {
            parts.append(sev)
        }
        parts.append(String(format: "(%.1f, %.1f, %.1f)", point.x, point.y, point.z))
        return parts.joined(separator: " • ")
    }
}

/// Element-level classification when comparing an active run against a baseline run.
enum PleDiffStatus {
    case unchanged
    case added
    case moved
    case removed
}

struct PleSceneDiff {
    let baselineTitle: String
    let addedIds: Set<String>
    let movedIds: Set<String>
    let removedElements: [PleSceneElement]
    let scoreDelta: Double?
    let lengthDelta: Double?
    let clashDelta: Int

    var addedCount: Int { addedIds.count }
    var movedCount: Int { movedIds.count }
    var removedCount: Int { removedElements.count }

    static func compute(active: PleScene, baseline: PleScene, baselineTitle: String) -> PleSceneDiff {
        var baseById: [String: PleSceneElement] = [:]
        for el in baseline.allElements {
            baseById[el.id] = el
        }
        var activeIds = Set<String>()
        var added = Set<String>()
        var moved = Set<String>()

        let posEps: Float = 0.015
        let sizeEps: Float = 0.015
        let rotEps: Float = 0.01

        for el in active.allElements {
            activeIds.insert(el.id)
            guard let prev = baseById[el.id] else {
                added.insert(el.id)
                continue
            }
            let dp = simd_length(el.centerVec - prev.centerVec)
            let ds = simd_length(el.sizeVec - prev.sizeVec)
            let dr = abs(el.rotZ - prev.rotZ)
            if dp > posEps || ds > sizeEps || dr > rotEps || el.shape.lowercased() != prev.shape.lowercased() || el.axisIndex != prev.axisIndex {
                moved.insert(el.id)
            }
        }

        var removed: [PleSceneElement] = []
        for el in baseline.allElements where !activeIds.contains(el.id) {
            removed.append(el)
        }

        let scoreDelta: Double?
        if let a = active.run?.score, let b = baseline.run?.score {
            scoreDelta = a - b
        } else {
            scoreDelta = nil
        }
        let lengthDelta: Double?
        if let a = active.run?.total_length_m, let b = baseline.run?.total_length_m {
            lengthDelta = a - b
        } else {
            lengthDelta = nil
        }
        let clashDelta = active.totalClashCount - baseline.totalClashCount

        return PleSceneDiff(
            baselineTitle: baselineTitle,
            addedIds: added,
            movedIds: moved,
            removedElements: removed,
            scoreDelta: scoreDelta,
            lengthDelta: lengthDelta,
            clashDelta: clashDelta
        )
    }

    func summaryText() -> String {
        var parts: [String] = []
        parts.append("vs \(baselineTitle)")
        parts.append("+\(addedCount) new")
        parts.append("~\(movedCount) moved")
        parts.append("-\(removedCount) removed")
        if let sd = scoreDelta {
            parts.append(String(format: "score %+.2f", sd))
        }
        if let ld = lengthDelta {
            parts.append(String(format: "len %+.1fm", ld))
        }
        if clashDelta != 0 {
            parts.append(String(format: "clashes %+d", clashDelta))
        } else {
            parts.append("clashes ±0")
        }
        return parts.joined(separator: " • ")
    }
}


// MARK: - Scene Loader & IFC Converter

enum PleSceneLoader {
    static func isSceneFile(_ url: URL) -> Bool {
        let lower = url.lastPathComponent.lowercased()
        return lower.hasSuffix(".ple-scene.json") || lower.hasSuffix(".ple-scene.v1.json")
    }

    static func isIfcFile(_ url: URL) -> Bool {
        url.pathExtension.lowercased() == "ifc"
    }

    static func load(from url: URL) throws -> PleScene {
        try PleScene.load(from: url)
    }

    static func loadIfc(from url: URL, searchRoot: URL? = nil) throws -> PleScene {
        let fm = FileManager.default
        var candidates: [URL] = []
        if let root = searchRoot {
            candidates.append(root.appendingPathComponent("tools/ple-ifc-to-scene.py"))
        }
        var dir = url.deletingLastPathComponent()
        for _ in 0..<8 {
            candidates.append(dir.appendingPathComponent("tools/ple-ifc-to-scene.py"))
            let parent = dir.deletingLastPathComponent()
            if parent.path == dir.path { break }
            dir = parent
        }
        candidates.append(URL(fileURLWithPath: fm.currentDirectoryPath).appendingPathComponent("tools/ple-ifc-to-scene.py"))
        guard let script = candidates.first(where: { fm.fileExists(atPath: $0.path) }) else {
            throw NSError(domain: "PleSceneLoader", code: 1, userInfo: [NSLocalizedDescriptionKey: "Could not find tools/ple-ifc-to-scene.py to convert \(url.lastPathComponent)."])
        }
        let tmp = fm.temporaryDirectory.appendingPathComponent("ple-scene-\(UUID().uuidString).ple-scene.json")
        defer { try? fm.removeItem(at: tmp) }
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        proc.arguments = ["python3", script.path, url.path, "-o", tmp.path]
        let errPipe = Pipe()
        proc.standardError = errPipe
        try proc.run()
        proc.waitUntilExit()
        guard proc.terminationStatus == 0, fm.fileExists(atPath: tmp.path) else {
            let errText = String(data: errPipe.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? "conversion failed"
            throw NSError(domain: "PleSceneLoader", code: 2, userInfo: [NSLocalizedDescriptionKey: "IFC conversion failed: \(errText.trimmingCharacters(in: .whitespacesAndNewlines))"])
        }
        return try PleScene.load(from: tmp)
    }
}
