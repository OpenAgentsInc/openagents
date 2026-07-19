#!/usr/bin/env swift

import Foundation
import ImageIO
import Vision

struct Arguments {
    var image: URL?
    var publicImage = "window.png"
}

struct OCRRow: Codable {
    let text: String
    let confidence: Float
    let x: Double
    let y: Double
    let width: Double
    let height: Double
}

struct ImageReference: Codable {
    let path: String
}

struct OCRResult: Codable {
    let provider: String
    let image: ImageReference
    let rows: [OCRRow]
}

func parseArguments() throws -> Arguments {
    var result = Arguments()
    var index = 1
    while index < CommandLine.arguments.count {
        guard index + 1 < CommandLine.arguments.count else {
            throw NSError(domain: "ui-gap-ocr", code: 2, userInfo: [NSLocalizedDescriptionKey: "An option has no value"])
        }
        let key = CommandLine.arguments[index]
        let value = CommandLine.arguments[index + 1]
        switch key {
        case "--image": result.image = URL(fileURLWithPath: value)
        case "--public-image": result.publicImage = value
        default:
            throw NSError(domain: "ui-gap-ocr", code: 2, userInfo: [NSLocalizedDescriptionKey: "Unknown option \(key)"])
        }
        index += 2
    }
    if result.image == nil {
        throw NSError(domain: "ui-gap-ocr", code: 2, userInfo: [NSLocalizedDescriptionKey: "Use --image"])
    }
    return result
}

func sanitized(_ value: String) -> String {
    var text = value
    let home = FileManager.default.homeDirectoryForCurrentUser.path
    if !home.isEmpty { text = text.replacingOccurrences(of: home, with: "<HOME>") }
    text = text.replacingOccurrences(of: NSTemporaryDirectory(), with: "<TMP>/")
    if text.hasPrefix("Log Out ") {
        text = text.hasSuffix("…") ? "Log Out <USER>…" : "Log Out <USER>"
    }
    return String(text.replacingOccurrences(of: "\n", with: " ").prefix(240))
}

do {
    let arguments = try parseArguments()
    guard
        let imageURL = arguments.image,
        let source = CGImageSourceCreateWithURL(imageURL as CFURL, nil),
        let image = CGImageSourceCreateImageAtIndex(source, 0, nil)
    else {
        throw NSError(domain: "ui-gap-ocr", code: 3, userInfo: [NSLocalizedDescriptionKey: "The image is not readable"])
    }

    let request = VNRecognizeTextRequest()
    request.recognitionLevel = .accurate
    request.usesLanguageCorrection = false
    request.recognitionLanguages = ["en-US"]
    try VNImageRequestHandler(cgImage: image).perform([request])

    let rows = (request.results ?? []).compactMap { observation -> OCRRow? in
        guard let candidate = observation.topCandidates(1).first else { return nil }
        let box = observation.boundingBox
        return OCRRow(
            text: sanitized(candidate.string),
            confidence: candidate.confidence,
            x: box.origin.x,
            y: box.origin.y,
            width: box.size.width,
            height: box.size.height
        )
    }.sorted {
        if abs($0.y - $1.y) > 0.01 { return $0.y > $1.y }
        return $0.x < $1.x
    }

    let encoder = JSONEncoder()
    encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
    let output = OCRResult(provider: "macos-vision", image: ImageReference(path: arguments.publicImage), rows: rows)
    FileHandle.standardOutput.write(try encoder.encode(output))
    FileHandle.standardOutput.write(Data("\n".utf8))
} catch {
    FileHandle.standardError.write(Data("\(error.localizedDescription)\n".utf8))
    exit(1)
}
