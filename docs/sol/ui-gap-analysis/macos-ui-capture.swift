#!/usr/bin/env swift

import AppKit
import ApplicationServices
import CoreGraphics
import CryptoKit
import Foundation
import ImageIO

struct Arguments {
    var pid: pid_t?
    var processName: String?
    var outputDirectory: URL?
    var windowTitle: String?
    var maxDepth = 14
    var maxNodes = 5000
}

func parseArguments() throws -> Arguments {
    var result = Arguments()
    var index = 1
    let values = CommandLine.arguments
    while index < values.count {
        let key = values[index]
        guard index + 1 < values.count else {
            throw NSError(domain: "ui-gap", code: 2, userInfo: [NSLocalizedDescriptionKey: "Missing value for \(key)"])
        }
        let value = values[index + 1]
        switch key {
        case "--pid": result.pid = pid_t(value)
        case "--process": result.processName = value
        case "--output-dir": result.outputDirectory = URL(fileURLWithPath: value, isDirectory: true)
        case "--window-title": result.windowTitle = value
        case "--max-depth": result.maxDepth = Int(value) ?? result.maxDepth
        case "--max-nodes": result.maxNodes = Int(value) ?? result.maxNodes
        default:
            throw NSError(domain: "ui-gap", code: 2, userInfo: [NSLocalizedDescriptionKey: "Unknown option \(key)"])
        }
        index += 2
    }
    if result.pid == nil && result.processName == nil {
        throw NSError(domain: "ui-gap", code: 2, userInfo: [NSLocalizedDescriptionKey: "Use --pid or --process"])
    }
    if result.outputDirectory == nil {
        throw NSError(domain: "ui-gap", code: 2, userInfo: [NSLocalizedDescriptionKey: "Use --output-dir"])
    }
    return result
}

func sanitized(_ value: String?) -> Any {
    guard var text = value, !text.isEmpty else { return NSNull() }
    let home = FileManager.default.homeDirectoryForCurrentUser.path
    if !home.isEmpty { text = text.replacingOccurrences(of: home, with: "<HOME>") }
    text = text.replacingOccurrences(of: NSTemporaryDirectory(), with: "<TMP>/")
    if text.hasPrefix("Log Out ") {
        text = text.hasSuffix("…") ? "Log Out <USER>…" : "Log Out <USER>"
    }
    text = text.replacingOccurrences(of: "\n", with: " ")
    text = text.replacingOccurrences(of: "\r", with: " ")
    return String(text.prefix(160))
}

func attribute(_ element: AXUIElement, _ name: CFString) -> CFTypeRef? {
    var value: CFTypeRef?
    let status = AXUIElementCopyAttributeValue(element, name, &value)
    return status == .success ? value : nil
}

func stringAttribute(_ element: AXUIElement, _ name: CFString) -> String? {
    return attribute(element, name) as? String
}

func boolAttribute(_ element: AXUIElement, _ name: CFString) -> Bool? {
    return attribute(element, name) as? Bool
}

func frameAttribute(_ element: AXUIElement) -> Any {
    guard
        let positionValue = attribute(element, kAXPositionAttribute as CFString),
        let sizeValue = attribute(element, kAXSizeAttribute as CFString),
        CFGetTypeID(positionValue) == AXValueGetTypeID(),
        CFGetTypeID(sizeValue) == AXValueGetTypeID()
    else { return NSNull() }
    var point = CGPoint.zero
    var size = CGSize.zero
    guard
        AXValueGetValue(positionValue as! AXValue, .cgPoint, &point),
        AXValueGetValue(sizeValue as! AXValue, .cgSize, &size)
    else { return NSNull() }
    return ["x": point.x, "y": point.y, "width": size.width, "height": size.height]
}

func actionNames(_ element: AXUIElement) -> [String] {
    var names: CFArray?
    guard AXUIElementCopyActionNames(element, &names) == .success else { return [] }
    return (names as? [String] ?? []).sorted()
}

func childElements(_ element: AXUIElement) -> [AXUIElement] {
    return attribute(element, kAXChildrenAttribute as CFString) as? [AXUIElement] ?? []
}

func safeValue(_ element: AXUIElement) -> Any {
    guard let value = attribute(element, kAXValueAttribute as CFString) else { return NSNull() }
    if let number = value as? NSNumber { return number }
    return NSNull()
}

func collectAccessibilityNodes(application: NSRunningApplication, maxDepth: Int, maxNodes: Int) -> ([[String: Any]], Bool) {
    guard AXIsProcessTrusted() else { return ([], false) }
    let root = AXUIElementCreateApplication(application.processIdentifier)
    var nodes: [[String: Any]] = []
    var queue: [(AXUIElement, String, Int)] = [(root, "0", 0)]
    var cursor = 0
    while cursor < queue.count && nodes.count < maxNodes {
        let (element, path, depth) = queue[cursor]
        cursor += 1
        let children = childElements(element)
        let role = stringAttribute(element, kAXRoleAttribute as CFString) ?? "AXUnknown"
        nodes.append([
            "path": path,
            "depth": depth,
            "role": role,
            "subrole": sanitized(stringAttribute(element, kAXSubroleAttribute as CFString)),
            "title": sanitized(stringAttribute(element, kAXTitleAttribute as CFString)),
            "description": sanitized(stringAttribute(element, kAXDescriptionAttribute as CFString)),
            "identifier": sanitized(stringAttribute(element, kAXIdentifierAttribute as CFString)),
            "value": safeValue(element),
            "enabled": boolAttribute(element, kAXEnabledAttribute as CFString) ?? NSNull(),
            "focused": boolAttribute(element, kAXFocusedAttribute as CFString) ?? NSNull(),
            "frame": frameAttribute(element),
            "actions": actionNames(element),
            "children": children.count,
        ])
        if depth < maxDepth {
            for (index, child) in children.enumerated() {
                queue.append((child, "\(path).\(index)", depth + 1))
            }
        }
    }
    return (nodes, cursor < queue.count)
}

func runningApplication(arguments: Arguments) throws -> NSRunningApplication {
    if let pid = arguments.pid, let application = NSRunningApplication(processIdentifier: pid) {
        return application
    }
    let name = arguments.processName?.lowercased() ?? ""
    if let application = NSWorkspace.shared.runningApplications.first(where: {
        ($0.localizedName?.lowercased() == name) || ($0.bundleIdentifier?.lowercased() == name)
    }) {
        return application
    }
    throw NSError(domain: "ui-gap", code: 3, userInfo: [NSLocalizedDescriptionKey: "The selected application is not running"])
}

func windowInformation(application: NSRunningApplication, titleFilter: String?) throws -> [String: Any] {
    guard let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] else {
        throw NSError(domain: "ui-gap", code: 4, userInfo: [NSLocalizedDescriptionKey: "The window server returned no window list"])
    }
    let candidates = windows.filter { window in
        let owner = (window[kCGWindowOwnerPID as String] as? NSNumber)?.int32Value
        let layer = (window[kCGWindowLayer as String] as? NSNumber)?.intValue ?? -1
        let alpha = (window[kCGWindowAlpha as String] as? NSNumber)?.doubleValue ?? 0
        let title = window[kCGWindowName as String] as? String ?? ""
        let titleMatches = titleFilter == nil || title.localizedCaseInsensitiveContains(titleFilter!)
        return owner == application.processIdentifier && layer == 0 && alpha > 0 && titleMatches
    }.sorted { left, right in
        let leftBounds = left[kCGWindowBounds as String] as? [String: Any] ?? [:]
        let rightBounds = right[kCGWindowBounds as String] as? [String: Any] ?? [:]
        let leftArea = ((leftBounds["Width"] as? NSNumber)?.doubleValue ?? 0) * ((leftBounds["Height"] as? NSNumber)?.doubleValue ?? 0)
        let rightArea = ((rightBounds["Width"] as? NSNumber)?.doubleValue ?? 0) * ((rightBounds["Height"] as? NSNumber)?.doubleValue ?? 0)
        return leftArea > rightArea
    }
    guard let window = candidates.first else {
        throw NSError(domain: "ui-gap", code: 4, userInfo: [NSLocalizedDescriptionKey: "No visible layer-zero window matched the selected application"])
    }
    guard
        let number = (window[kCGWindowNumber as String] as? NSNumber)?.uint32Value,
        let boundsDictionary = window[kCGWindowBounds as String] as? [String: Any],
        let bounds = CGRect(dictionaryRepresentation: boundsDictionary as CFDictionary)
    else {
        throw NSError(domain: "ui-gap", code: 4, userInfo: [NSLocalizedDescriptionKey: "The selected window has invalid metadata"])
    }
    return [
        "id": Int(number),
        "title": (sanitized(window[kCGWindowName as String] as? String) as? String) ?? "",
        "frame": ["x": bounds.origin.x, "y": bounds.origin.y, "width": bounds.width, "height": bounds.height],
    ]
}

func captureWindow(windowID: Int, output: URL) -> String? {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
    process.arguments = ["-x", "-l", String(windowID), output.path]
    do {
        try process.run()
        process.waitUntilExit()
        return process.terminationStatus == 0 && FileManager.default.fileExists(atPath: output.path)
            ? nil
            : "screencapture did not produce an image. Check Screen Recording permission."
    } catch {
        return "screencapture failed: \(error.localizedDescription)"
    }
}

func fileSHA256(_ url: URL) throws -> String {
    let data = try Data(contentsOf: url)
    return SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
}

func imageMetrics(_ url: URL) throws -> [String: Any] {
    guard
        let source = CGImageSourceCreateWithURL(url as CFURL, nil),
        let image = CGImageSourceCreateImageAtIndex(source, 0, nil)
    else {
        throw NSError(domain: "ui-gap", code: 5, userInfo: [NSLocalizedDescriptionKey: "The screenshot is not a readable image"])
    }
    let representation = NSBitmapImageRep(cgImage: image)
    let width = representation.pixelsWide
    let height = representation.pixelsHigh
    let sampleStride = max(1, Int(sqrt(Double(width * height) / 250_000.0)))
    var lumas: [Double] = []
    var histogram: [Int: Int] = [:]
    var edgeCount = 0
    var edgeTotal = 0
    var priorRow: [Double] = []
    var y = 0
    while y < height {
        var currentRow: [Double] = []
        var x = 0
        while x < width {
            guard let color = representation.colorAt(x: x, y: y)?.usingColorSpace(.deviceRGB) else {
                x += sampleStride
                continue
            }
            let red = Double(color.redComponent)
            let green = Double(color.greenComponent)
            let blue = Double(color.blueComponent)
            let luma = 0.2126 * red + 0.7152 * green + 0.0722 * blue
            if let previous = currentRow.last {
                edgeTotal += 1
                if abs(luma - previous) >= 0.12 { edgeCount += 1 }
            }
            if currentRow.count < priorRow.count {
                edgeTotal += 1
                if abs(luma - priorRow[currentRow.count]) >= 0.12 { edgeCount += 1 }
            }
            currentRow.append(luma)
            lumas.append(luma)
            let redBin = min(7, Int(red * 8.0))
            let greenBin = min(7, Int(green * 8.0))
            let blueBin = min(7, Int(blue * 8.0))
            let key = (redBin << 6) | (greenBin << 3) | blueBin
            histogram[key, default: 0] += 1
            x += sampleStride
        }
        priorRow = currentRow
        y += sampleStride
    }
    guard !lumas.isEmpty else {
        throw NSError(domain: "ui-gap", code: 5, userInfo: [NSLocalizedDescriptionKey: "The screenshot has no sampleable pixels"])
    }
    let mean = lumas.reduce(0, +) / Double(lumas.count)
    let variance = lumas.reduce(0) { $0 + pow($1 - mean, 2) } / Double(lumas.count)
    let sorted = lumas.sorted()
    let percentile: (Double) -> Double = { fraction in
        sorted[min(sorted.count - 1, Int(Double(sorted.count - 1) * fraction))]
    }
    let dominantColors = histogram.sorted { $0.value > $1.value }.prefix(12).map { entry -> [String: Any] in
        let red = ((entry.key >> 6) & 7) * 32 + 16
        let green = ((entry.key >> 3) & 7) * 32 + 16
        let blue = (entry.key & 7) * 32 + 16
        return [
            "hex": String(format: "#%02X%02X%02X", red, green, blue),
            "share": Double(entry.value) / Double(lumas.count),
        ]
    }
    let attributes = try FileManager.default.attributesOfItem(atPath: url.path)
    return [
        "path": url.lastPathComponent,
        "sha256": try fileSHA256(url),
        "bytes": attributes[.size] as? NSNumber ?? 0,
        "width": width,
        "height": height,
        "meanLuma": mean,
        "lumaStandardDeviation": sqrt(variance),
        "lumaP10": percentile(0.10),
        "lumaP90": percentile(0.90),
        "edgeDensity": edgeTotal == 0 ? 0 : Double(edgeCount) / Double(edgeTotal),
        "dominantColors": dominantColors,
    ]
}

do {
    let arguments = try parseArguments()
    let outputDirectory = arguments.outputDirectory!
    try FileManager.default.createDirectory(at: outputDirectory, withIntermediateDirectories: true)
    let application = try runningApplication(arguments: arguments)
    let window = try windowInformation(application: application, titleFilter: arguments.windowTitle)
    let windowID = window["id"] as! Int
    let screenshot = outputDirectory.appendingPathComponent("window.png")
    let screenshotLimitation = captureWindow(windowID: windowID, output: screenshot)
    let visual: Any
    var limitations: [String] = []
    if screenshotLimitation == nil {
        visual = try imageMetrics(screenshot)
    } else {
        visual = NSNull()
        limitations.append(screenshotLimitation!)
    }
    let (nodes, truncated) = collectAccessibilityNodes(
        application: application,
        maxDepth: arguments.maxDepth,
        maxNodes: arguments.maxNodes
    )
    if !AXIsProcessTrusted() {
        limitations.append("The capture process does not have macOS Accessibility permission. The accessibility node set is empty.")
    }
    let result: [String: Any] = [
        "platform": "macOS-\(ProcessInfo.processInfo.operatingSystemVersionString)",
        "process": [
            "pid": Int(application.processIdentifier),
            "name": application.localizedName ?? "unknown",
            "bundleIdentifier": sanitized(application.bundleIdentifier),
        ],
        "window": window,
        "accessibility": ["trusted": AXIsProcessTrusted(), "truncated": truncated, "nodes": nodes],
        "visual": visual,
        "limitations": limitations,
    ]
    let data = try JSONSerialization.data(withJSONObject: result, options: [.prettyPrinted, .sortedKeys])
    FileHandle.standardOutput.write(data)
    FileHandle.standardOutput.write(Data("\n".utf8))
} catch {
    FileHandle.standardError.write(Data("\(error.localizedDescription)\n".utf8))
    exit(1)
}
