import UIKit
import Metal
import AVFAudio
import UniformTypeIdentifiers

func hauntText(_ value: coax_string) -> String {
    guard let bytes = value.data, value.count > 0 else { return "" }
    return String(decoding: UnsafeBufferPointer(start: UnsafeRawPointer(bytes).assumingMemoryBound(to: UInt8.self), count: Int(value.count)), as: UTF8.self)
}
func hauntPoint(_ v: coax_t_Vec3) -> coax_t_HauntPoint { .init(x: v.x, y: v.y, z: v.z) }

/// Platform resource/memory host. Every array has a fixed capacity, and every
/// protobuf/string view is backed by memory retained until this host dies.
final class HauntAssets {
    var state = coax_t_Haunt()
    let vertices: MTLBuffer, indices: MTLBuffer, sheets: MTLTexture
    let instances: [MTLBuffer], lightBuffers: [MTLBuffer]
    let drawPackets = UnsafeMutablePointer<coax_t_HauntDraw>.allocate(capacity: 256)
    private var allocations: [UnsafeMutableRawPointer] = []
    private var retainedBytes: [UnsafeMutableRawBufferPointer] = []
    private var soundPlayers: [AVAudioPlayer] = []
    private var ambience: AVAudioPlayer?
    private var undoStates: [Data] = [], redoStates: [Data] = []
    private let output = UnsafeMutablePointer<UInt8>.allocate(capacity: 262144)
    private let scratch = UnsafeMutablePointer<UInt8>.allocate(capacity: 262144)
    private var saveURL: URL
    var failure: String?
    var wallBytes = [Float](repeating: 0, count: 4096 * 6)
    var wallCount = 0

    private func storage<T>(_ type: T.Type, count: Int) -> UnsafeMutablePointer<T> {
        let pointer = UnsafeMutableRawPointer.allocate(byteCount: count * MemoryLayout<T>.stride, alignment: max(16, MemoryLayout<T>.alignment))
        pointer.initializeMemory(as: UInt8.self, repeating: 0, count: count * MemoryLayout<T>.stride)
        allocations.append(pointer)
        return pointer.bindMemory(to: T.self, capacity: count)
    }
    func retain(_ data: Data) -> coax_v_u8 {
        let b = UnsafeMutableRawBufferPointer.allocate(byteCount: max(data.count, 1), alignment: 16)
        data.copyBytes(to: b); retainedBytes.append(b)
        return coax_v_u8(data: b.baseAddress!.assumingMemoryBound(to: UInt8.self), count: Int64(data.count))
    }
    func retain(_ string: String) -> coax_string {
        let b = retain(Data(string.utf8))
        return coax_string(data: UnsafeRawPointer(b.data!).assumingMemoryBound(to: CChar.self), count: b.count)
    }
    init(device: MTLDevice, queue: MTLCommandQueue) throws {
        let directory = try FileManager.default.url(for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
        let active = UserDefaults.standard.string(forKey: "haunt.active") ?? "HauntedHouse-v1.pb"
        saveURL = directory.appendingPathComponent(active.hasPrefix("HauntedHouse-") && !active.contains("/") ? active : "HauntedHouse-v1.pb")
        func resource(_ name: String, _ ext: String) throws -> Data {
            guard let url = Bundle.main.url(forResource: name, withExtension: ext) else { throw HomeError.resource(name + "." + ext) }
            return try Data(contentsOf: url)
        }
        let vertexData = try resource("Haunt", "vertices"), indexData = try resource("Haunt", "indices")
        let layers = [0, 1, 2, 18, 19, 20, 21, 22, 24, 26, 27, 28]
        let valid = vertexData.withUnsafeBytes { v in indexData.withUnsafeBytes { i in
            metareal_haunt_buffer_ok(coax_v_u8(data: UnsafeMutablePointer(mutating: v.bindMemory(to: UInt8.self).baseAddress), count: Int64(v.count)), coax_v_u8(data: UnsafeMutablePointer(mutating: i.bindMemory(to: UInt8.self).baseAddress), count: Int64(i.count)), Int64(layers.count))
        } }
        guard valid, let vertices = vertexData.withUnsafeBytes({ device.makeBuffer(bytes: $0.baseAddress!, length: $0.count, options: .storageModeShared) }),
              let indices = indexData.withUnsafeBytes({ device.makeBuffer(bytes: $0.baseAddress!, length: $0.count, options: .storageModeShared) }) else { throw HomeError.resource("Haunted House geometry") }
        self.vertices = vertices; self.indices = indices
        let side = 1024
        let descriptor = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .rgba8Unorm_srgb, width: side, height: side, mipmapped: true)
        descriptor.textureType = .type2DArray; descriptor.arrayLength = layers.count; descriptor.usage = .shaderRead; descriptor.storageMode = .shared
        guard let sheets = device.makeTexture(descriptor: descriptor) else { throw HomeError.resource("House textures") }
        var pixels = [UInt8](repeating: 0, count: side * side * 4)
        for (slice, number) in layers.enumerated() {
            guard let url = Bundle.main.url(forResource: "haunt-layer\(number)", withExtension: "png") ?? Bundle.main.url(forResource: "haunt-layer\(number)", withExtension: "jpg"),
                  let image = UIImage(contentsOfFile: url.path)?.cgImage else { throw HomeError.resource("House texture \(number)") }
            let drawn = pixels.withUnsafeMutableBytes { bytes -> Bool in
                guard let c = CGContext(data: bytes.baseAddress, width: side, height: side, bitsPerComponent: 8, bytesPerRow: side * 4, space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return false }
                c.clear(CGRect(x: 0, y: 0, width: side, height: side))
                c.draw(image, in: CGRect(x: 0, y: 0, width: side, height: side)); return true
            }
            guard drawn else { throw HomeError.resource("House texture upload") }
            pixels.withUnsafeBytes { sheets.replace(region: MTLRegionMake2D(0, 0, side, side), mipmapLevel: 0, slice: slice, withBytes: $0.baseAddress!, bytesPerRow: side * 4, bytesPerImage: side * side * 4) }
        }
        guard let command = queue.makeCommandBuffer(), let blit = command.makeBlitCommandEncoder() else { throw HomeError.resource("House texture mipmaps") }
        blit.generateMipmaps(for: sheets); blit.endEncoding(); command.commit()
        self.sheets = sheets
        var inst: [MTLBuffer] = [], lights: [MTLBuffer] = []
        for _ in 0..<3 {
            guard let a = device.makeBuffer(length: 4096 * MemoryLayout<coax_t_HauntInstance>.stride, options: .storageModeShared),
                  let b = device.makeBuffer(length: 64 * MemoryLayout<coax_t_HauntGPUlight>.stride, options: .storageModeShared) else { throw HomeError.resource("House frame buffers") }
            inst.append(a); lights.append(b)
        }
        instances = inst; lightBuffers = lights
        state.meshes = coax_v_t_HauntMesh(data: storage(coax_t_HauntMesh.self, count: 256), count: 256)
        state.entities = coax_v_t_HauntEntity(data: storage(coax_t_HauntEntity.self, count: 4096), count: 4096)
        state.areas = coax_v_t_HauntArea(data: storage(coax_t_HauntArea.self, count: 64), count: 64)
        state.walks = coax_v_t_HauntRect(data: storage(coax_t_HauntRect.self, count: 256), count: 256)
        state.lights = coax_v_t_HauntLight(data: storage(coax_t_HauntLight.self, count: 63), count: 63)
        state.people = coax_v_t_HauntPerson(data: storage(coax_t_HauntPerson.self, count: 64), count: 64)
        state.zones = coax_v_t_HauntZone(data: storage(coax_t_HauntZone.self, count: 32), count: 32)
        state.actions = coax_v_t_HauntAction(data: storage(coax_t_HauntAction.self, count: 256), count: 256)
        state.pending = coax_v_t_HauntEvent(data: storage(coax_t_HauntEvent.self, count: 256), count: 256)
        state.talking = -1; state.selected = -1; state.holding = -1; state.was_ground = true
        state = metareal_haunt_open(retain(try resource("Haunt", "pb")), state)
        guard state.ok else { throw HomeError.resource("Haunted House document") }
        for i in 0..<Int(state.mesh_count) { let mesh = state.meshes.data![i]
            guard metareal_haunt_draw_range(Int64(mesh.first), Int64(mesh.count), Int64(indexData.count / 4)) else { throw HomeError.resource("House draw range") }
        }
        if let fileSize = try? saveURL.resourceValues(forKeys: [.fileSizeKey]).fileSize, fileSize <= 262144, let saved = try? Data(contentsOf: saveURL) { state = metareal_haunt_restore(state, retain(saved)) }
        rebuildWalls()
    }
    func rebuildWalls() {
        wallCount = wallBytes.withUnsafeMutableBufferPointer { Int(metareal_haunt_walls(state, coax_v_f32(data: $0.baseAddress, count: Int64($0.count)))) }
    }
    func snapshot() -> Data? {
        let n = metareal_haunt_save(state, coax_v_u8(data: output, count: 262144), coax_v_u8(data: scratch, count: 262144))
        return n > 0 ? Data(bytes: output, count: Int(n)) : nil
    }
    func remember() { if let data = snapshot() { undoStates.append(data); if undoStates.count > 30 { undoStates.removeFirst() }; redoStates.removeAll() } }
    func undo() { guard let data = undoStates.popLast() else { return }; if let now = snapshot() { redoStates.append(now) }; state = metareal_haunt_restore(state, retain(data)); rebuildWalls(); save() }
    func redo() { guard let data = redoStates.popLast() else { return }; if let now = snapshot() { undoStates.append(now) }; state = metareal_haunt_restore(state, retain(data)); rebuildWalls(); save() }
    func save() {
        guard let data = snapshot() else { failure = "Could not encode this world."; return }
        do { try data.write(to: saveURL, options: .atomic); state.dirty = false } catch { failure = "Could not save this world." }
    }
    func importWorld(_ url: URL) throws {
        let access = url.startAccessingSecurityScopedResource(); defer { if access { url.stopAccessingSecurityScopedResource() } }
        let values = try url.resourceValues(forKeys: [.fileSizeKey])
        guard (values.fileSize ?? 262145) <= 262144 else { throw HomeError.resource("World file too large") }
        let data = try Data(contentsOf: url)
        let bytes = retain(data)
        guard metareal_haunt_save_valid(state, bytes) else { throw HomeError.resource("Unsupported or damaged world file") }
        remember()
        state = metareal_haunt_restore(state, bytes); rebuildWalls(); save()
    }
    func createWorld(_ title: String) {
        let id = UUID().uuidString
        let next = metareal_haunt_fork(state,retain(title),retain(id))
        guard hauntText(next.instance_id) == id else { failure = "Choose a shorter world name."; return }
        save()
        saveURL = saveURL.deletingLastPathComponent().appendingPathComponent("HauntedHouse-"+id+".pb")
        UserDefaults.standard.set(saveURL.lastPathComponent, forKey: "haunt.active")
        state = next; save()
    }
    func worlds() -> [(URL,String)] {
        let paths = (try? FileManager.default.contentsOfDirectory(at:saveURL.deletingLastPathComponent(),includingPropertiesForKeys:[.fileSizeKey])) ?? []
        return paths.filter { $0.lastPathComponent.hasPrefix("HauntedHouse-") && $0.pathExtension == "pb" }.compactMap { url in
            guard let size = try? url.resourceValues(forKeys:[.fileSizeKey]).fileSize, size <= 262144, let data = try? Data(contentsOf:url) else { return nil }
            let name = data.withUnsafeBytes { bytes in hauntText(metareal_haunt_saved_title(coax_v_u8(data:UnsafeMutablePointer(mutating:bytes.bindMemory(to:UInt8.self).baseAddress),count:Int64(bytes.count)))) }
            return name.isEmpty ? nil : (url,name)
        }
    }
    func switchWorld(_ url: URL) throws {
        let data = try Data(contentsOf:url), bytes = retain(data)
        guard metareal_haunt_save_valid(state,bytes) else { throw HomeError.resource("Unsupported world") }
        save();state = metareal_haunt_restore(state,bytes);saveURL=url
        UserDefaults.standard.set(url.lastPathComponent,forKey:"haunt.active");undoStates=[];redoStates=[];rebuildWalls()
    }
    func exportWorld() throws -> URL {
        guard let data = snapshot() else { throw HomeError.resource("World could not be encoded") }
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("HauntedHouse.openagents-world.pb")
        try data.write(to: url, options: .atomic); return url
    }
    func reset() throws {
        remember()
        let views = state
        state = coax_t_Haunt(); state.meshes = views.meshes; state.entities = views.entities; state.areas = views.areas; state.walks = views.walks; state.lights = views.lights; state.people = views.people; state.zones = views.zones; state.actions = views.actions; state.pending = views.pending
        state.talking = -1; state.selected = -1; state.holding = -1; state.was_ground = true
        let data = try Data(contentsOf: Bundle.main.url(forResource: "Haunt", withExtension: "pb")!)
        state = metareal_haunt_open(retain(data), state); rebuildWalls(); save()
    }
    func startAudio() {
        do {
            try AVAudioSession.sharedInstance().setCategory(.ambient, mode: .default, options: [.mixWithOthers])
            try AVAudioSession.sharedInstance().setActive(true)
            if ambience == nil, let url = Bundle.main.url(forResource: "haunt-wind_loop", withExtension: "mp3") { ambience = try AVAudioPlayer(contentsOf: url); ambience?.numberOfLoops = -1; ambience?.volume = 0.35 }
            ambience?.play()
        } catch { failure = "Audio unavailable." }
    }
    func stopAudio() { ambience?.pause(); for p in soundPlayers { p.stop() }; soundPlayers.removeAll() }
    func playSounds() {
        soundPlayers.removeAll { !$0.isPlaying }
        withUnsafePointer(to: state.sounds.e) { pointer in pointer.withMemoryRebound(to: coax_t_HauntSound.self, capacity: 16) { sounds in
            for i in 0..<Int(state.sound_count) {
                let cue = sounds[i]
                guard cue.gain > 0, let url = Bundle.main.url(forResource: "haunt-" + hauntText(cue.name), withExtension: "mp3"), let player = try? AVAudioPlayer(contentsOf: url) else { continue }
                player.volume = cue.gain; player.pan = cue.pan; player.play(); soundPlayers.append(player)
            }
        } }
        if soundPlayers.count > 16 { for p in soundPlayers.prefix(soundPlayers.count - 16) { p.stop() }; soundPlayers.removeFirst(soundPlayers.count - 16) }
    }
    deinit { stopAudio(); for a in allocations { a.deallocate() }; for b in retainedBytes { b.deallocate() }; output.deallocate(); scratch.deallocate(); drawPackets.deallocate() }
}
