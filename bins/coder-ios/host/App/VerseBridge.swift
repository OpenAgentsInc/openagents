// Verse's native mount is independent of the reader's background worker.
// Rust owns world state, movement, rendering, and relay effects.
import Foundation
import QuartzCore
import SwiftUI

struct VersePacket: Decodable {
    let schema: String
    let status: String
    let error: String?
    let frames_presented: UInt64
    let position: [Double]
    let view: NativeView?
}

@MainActor
final class VerseBridge: ObservableObject {
    @Published private(set) var packet: VersePacket?
    @Published private(set) var nativeError: String?
    let synthetic: Bool
    private weak var canvas: VerseMetalView?
    private var lastPublished: CFTimeInterval = 0

    init(synthetic: Bool) {
        self.synthetic = synthetic
        do { packet = try Self.decode(coder_verse_blueprint()) }
        catch { nativeError = error.localizedDescription }
    }

    func bind(_ canvas: VerseMetalView) { self.canvas = canvas }
    func unbind(_ canvas: VerseMetalView) {
        if self.canvas === canvas { self.canvas = nil }
    }

    func send(_ request: [String: Any]) {
        guard let canvas else {
            nativeError = "The world surface is not mounted. Open Verse again."
            return
        }
        canvas.send(request, forcePublish: true)
    }

    func retry() { canvas?.recreate() }

    func receive(_ result: Result<VersePacket, Error>, from source: VerseMetalView, force: Bool) {
        guard canvas === source else { return }
        switch result {
        case let .success(packet):
            let now = CACurrentMediaTime()
            guard force || packet.error != nil || now - lastPublished >= 0.15 else { return }
            lastPublished = now
            self.packet = packet
            nativeError = nil
        case let .failure(error): nativeError = error.localizedDescription
        }
    }

    static func decode(_ result: CoderMobileBuffer) throws -> VersePacket {
        defer { coder_mobile_buffer_free(result) }
        guard let data = result.data, result.len > 0, result.len <= 1_048_576 else {
            throw ReaderError.message("The world returned an invalid view packet.")
        }
        let packet = try JSONDecoder().decode(VersePacket.self, from: Data(bytes: data, count: result.len))
        guard packet.schema == "coder.verse.v1",
              packet.position.count == 3, packet.position.allSatisfy(\.isFinite) else {
            throw ReaderError.message("This app does not support the returned world view.")
        }
        guard let view = packet.view else {
            throw ReaderError.message(packet.error ?? "The world view is unavailable.")
        }
        guard view.schema == "rust-native.view.v2" else {
            throw ReaderError.message("This app does not support the returned world view version.")
        }
        return packet
    }
}
