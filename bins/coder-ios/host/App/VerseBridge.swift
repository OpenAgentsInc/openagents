// Verse's native mount is independent of the reader's background worker.
// Rust owns world state, movement, rendering, and relay effects.
import Foundation
import QuartzCore
import SwiftUI

struct VersePacket: Decodable {
    let schema: String
    let status: String
    let connection: VerseConnection
    let error: String?
    let frames_presented: UInt64
    let position: [Double]
    let view: NativeView?
    let computer: VerseComputer
    let computer_open: Bool
    let gym: VerseGym
    let gym_open: Bool
    let gym_active: Bool
    let gym_revision: UInt64
    let gym_board: GymBoardView?
    let camera_mode: String
    let camera_yaw: Double
    let camera_pitch: Double
    let camera_distance: Double
    let motion_needed: Bool
    let map: VerseMap
    let companion: VerseCompanion
    let doors: VerseDoors
    let door_preferences: String
    let door_preferences_revision: UInt64
}

struct VerseMap: Decodable {
    let visible: Bool
    let expanded: Bool
    let state: String
    let destination: [Double]?
    let captured_pointers: [UInt64]
    let frame: [Double]
    let plot: [Double]
    let center: [Double]
    let half_extent: Double
    let landmarks: [VerseMapLandmark]

    var valid: Bool {
        frame.count == 4 && plot.count == 4 && center.count == 2 &&
        frame.allSatisfy(\.isFinite) && plot.allSatisfy(\.isFinite) && center.allSatisfy(\.isFinite) &&
        frame[2] >= 0 && frame[3] >= 0 && plot[2] >= 0 && plot[3] >= 0 &&
        half_extent.isFinite && half_extent > 0 && state.utf8.count <= 512 &&
        captured_pointers.count <= 8 && Set(captured_pointers).count == captured_pointers.count &&
        (destination == nil || (destination?.count == 2 && destination?.allSatisfy(\.isFinite) == true)) &&
        landmarks.count <= 256 && landmarks.allSatisfy { $0.x.isFinite && $0.z.isFinite && $0.id.utf8.count <= 128 && $0.label.utf8.count <= 128 }
    }

    var observation: [String: Any] {
        ["visible": visible, "expanded": expanded, "state": state,
         "destination": destination as Any? ?? NSNull(), "captured_pointers": captured_pointers,
         "frame": frame, "plot": plot, "center": center, "half_extent": half_extent,
         "landmarks": landmarks.map { ["id": $0.id, "label": $0.label, "x": $0.x, "z": $0.z] as [String: Any] }]
    }
}

struct VerseMapLandmark: Decodable {
    let id: String
    let label: String
    let x: Double
    let z: Double
}

struct VerseCompanion: Decodable {
    let near: Bool
    let visible: Bool
    let screen_x: Double
    let screen_y: Double
    let distance: Double
    let reacting: Bool
    let cooldown_seconds: Double
    let pet_count: UInt64

    var valid: Bool {
        screen_x.isFinite && screen_y.isFinite && (0...1).contains(screen_x) &&
        (0...1).contains(screen_y) && distance.isFinite && distance >= 0 &&
        cooldown_seconds.isFinite && cooldown_seconds >= 0
    }
    var observation: [String: Any] {
        ["near": near, "visible": visible, "screen_x": screen_x, "screen_y": screen_y,
         "distance": distance, "reacting": reacting, "cooldown_seconds": cooldown_seconds,
         "pet_count": pet_count]
    }
}

struct VerseConnection: Decodable {
    let state: String
    let label: String
    let relay: String?
    let error: String?
}

struct VerseComputer: Decodable {
    let near: Bool
    let visible: Bool
    let screen_x: Double
    let screen_y: Double
    let distance: Double
}

struct VerseGym: Decodable {
    let inside: Bool
    let near: Bool
    let visible: Bool
    let screen_x: Double
    let screen_y: Double
    let distance: Double
}

@MainActor
final class VerseBridge: ObservableObject {
    @Published private(set) var packet: VersePacket?
    @Published private(set) var nativeError: String?
    @Published private(set) var gymBoard: GymBoardView?
    @Published private(set) var doorStorageError: String?
    @Published private(set) var canRetryDoorSave = false
    private var attemptedDoorRevision: UInt64?
    private var latestDoorDocument: String?
    private(set) var doorStorageWrites: UInt64 = 0
    @Published private(set) var worldStorageError: String?
    @Published private(set) var gymStorageError: String?
    @Published private(set) var motionError: String?
    let motionAvailable: Bool
    let motionSynthetic: Bool
    let motionDriver: DeviceMotionDriver
    private let motionPreview: PreviewDeviceMotionSource?
    let synthetic: Bool
    private weak var canvas: VerseMetalView?
    private var lastPublished: CFTimeInterval = 0
    private var gymRequestedRevision: UInt64?

    init(synthetic: Bool) {
        self.synthetic = synthetic
        motionSynthetic = synthetic && ProcessInfo.processInfo.arguments.contains("--motion-preview")
        let preview = motionSynthetic ? PreviewDeviceMotionSource() : nil
        motionPreview = preview
        let source: DeviceMotionSource
        if let preview { source = preview } else { source = CoreMotionSource() }
        motionDriver = DeviceMotionDriver(source: source)
        motionAvailable = motionDriver.available
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
        let result = canvas.send(request, forcePublish: true)
        if let action = request["action"] as? String, ["connect", "disconnect"].contains(action),
           case let .success(packet) = result, packet.error == nil {
            do {
                try WorldConnection.save(packet.connection.relay, synthetic: synthetic)
                worldStorageError = nil
            } catch { worldStorageError = error.localizedDescription }
        }
    }

    func retry() { canvas?.recreate() }

    func retryDoorPreferences() {
        guard canRetryDoorSave, let document = latestDoorDocument else { return }
        do {
            try DoorPreferences.save(document, synthetic: synthetic)
            doorStorageWrites += 1
            doorStorageError = nil
            canRetryDoorSave = false
        } catch { doorStorageError = error.localizedDescription }
    }

    func storedDoorPreferences() -> String? {
        attemptedDoorRevision = nil
        latestDoorDocument = nil
        doorStorageWrites = 0
        canRetryDoorSave = false
        do {
            let document = try DoorPreferences.load(synthetic: synthetic)
            doorStorageError = nil
            return document
        } catch { doorStorageError = error.localizedDescription; return nil }
    }

    func retainDoorPreferences(_ packet: VersePacket, from source: VerseMetalView) {
        guard canvas === source else { return }
        latestDoorDocument = packet.door_preferences
        guard let previous = attemptedDoorRevision else {
            // Initial decode establishes the revision without rewriting invalid
            // or unavailable saved data with a default document.
            attemptedDoorRevision = packet.door_preferences_revision
            return
        }
        guard previous != packet.door_preferences_revision else { return }
        attemptedDoorRevision = packet.door_preferences_revision
        do {
            try DoorPreferences.save(packet.door_preferences, synthetic: synthetic)
            doorStorageWrites += 1
            doorStorageError = nil
            canRetryDoorSave = false
        } catch { doorStorageError = error.localizedDescription; canRetryDoorSave = true }
    }

    func toggleCameraMode() {
        if packet?.camera_mode == "motion" {
            send(["action": "camera_mode", "mode": "touch"])
            motionError = nil
        } else if motionAvailable {
            motionError = nil
            send(["action": "camera_mode", "mode": "motion"])
        } else {
            motionError = DeviceMotionFailure.unavailable.localizedDescription
        }
    }

    func recenterMotion() { send(["action": "recenter_camera"]) }

    func previewMotion() {
        guard motionSynthetic, packet?.camera_mode == "motion" else { return }
        motionPreview?.advance()
    }

    func reportMotionFailure(_ error: Error) { motionError = error.localizedDescription }

    func storedWorldRelay() -> String? {
        do { return try WorldConnection.load(synthetic: synthetic) }
        catch { worldStorageError = error.localizedDescription; return nil }
    }

    func storedGymCode() -> String? {
        do { return try GymConnection.load(synthetic: synthetic) }
        catch { gymStorageError = error.localizedDescription; return nil }
    }

    func configureGym(_ code: String) -> Bool {
        guard code.utf8.count <= 65_536 else {
            nativeError = "The Gym connection exceeds its size limit."
            return false
        }
        guard let result = canvas?.send(["action": "gym_configure", "code": code], forcePublish: true),
              case let .success(packet) = result, packet.error == nil,
              packet.gym_board?.configured == true else { return false }
        do { try GymConnection.save(code, synthetic: synthetic); gymStorageError = nil }
        catch { gymStorageError = error.localizedDescription }
        return true
    }

    func receive(_ result: Result<VersePacket, Error>, from source: VerseMetalView, force: Bool) {
        guard canvas === source else { return }
        switch result {
        case let .success(packet):
            let now = CACurrentMediaTime()
            guard force || packet.error != nil || now - lastPublished >= 0.15 else { return }
            lastPublished = now
            self.packet = packet
            nativeError = nil
            if !packet.gym_active || !packet.gym.inside {
                gymBoard = nil
                gymRequestedRevision = nil
            } else if let board = packet.gym_board {
                gymBoard = board
                gymRequestedRevision = board.revision
            }
            if packet.gym_active, packet.gym_open, packet.gym.inside,
               gymBoard?.revision != packet.gym_revision,
               gymRequestedRevision != packet.gym_revision {
                gymRequestedRevision = packet.gym_revision
                DispatchQueue.main.async { [weak self, weak source] in
                    guard let self, let source, self.canvas === source,
                          self.packet?.gym_active == true, self.packet?.gym_open == true else { return }
                    source.send(["action": "gym_view"], forcePublish: true)
                }
            }
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
              ["offline", "paused", "connecting", "connected", "retrying", "preview"].contains(packet.connection.state),
              (packet.connection.relay?.utf8.count ?? 0) <= 2048,
              packet.position.count == 3, packet.position.allSatisfy(\.isFinite),
              packet.computer.screen_x.isFinite, packet.computer.screen_y.isFinite,
              packet.computer.distance.isFinite,
              packet.gym.screen_x.isFinite, packet.gym.screen_y.isFinite, packet.gym.distance.isFinite,
              ["touch", "motion"].contains(packet.camera_mode),
              packet.camera_yaw.isFinite, packet.camera_pitch.isFinite,
              packet.camera_distance.isFinite, packet.camera_distance > 0,
              packet.map.valid, packet.companion.valid, packet.door_preferences.utf8.count <= 2048, packet.doors.valid,
              packet.gym_board?.valid ?? true else {
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
