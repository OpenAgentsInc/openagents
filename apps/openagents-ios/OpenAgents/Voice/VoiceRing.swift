import Synchronization

/// A single-producer, single-consumer ring of 16-bit samples. The microphone
/// tap (or the fixture feeder) writes; the voice worker reads. Storage is
/// allocated once; `write` and `read` never lock or allocate, so the tap's
/// only work is a copy. When the worker falls behind, the oldest audio is not
/// overwritten: new samples are counted as dropped instead.
final class VoiceRing: @unchecked Sendable {
    private let storage: UnsafeMutablePointer<Int16>
    private let capacity: Int
    private let head = Atomic<Int>(0) // total samples written
    private let tail = Atomic<Int>(0) // total samples read
    let dropped = Atomic<Int>(0)

    init(capacity: Int) {
        self.capacity = capacity
        storage = .allocate(capacity: capacity)
        storage.initialize(repeating: 0, count: capacity)
    }
    deinit { storage.deallocate() }

    var available: Int { head.load(ordering: .acquiring) - tail.load(ordering: .acquiring) }

    /// Producer side. Writes as many samples as fit.
    @discardableResult
    func write(_ samples: UnsafePointer<Int16>, count: Int) -> Int {
        let h = head.load(ordering: .relaxed), t = tail.load(ordering: .acquiring)
        let n = min(count, capacity - (h - t))
        if n < count { dropped.wrappingAdd(count - n, ordering: .relaxed) }
        guard n > 0 else { return 0 }
        let at = h % capacity, first = min(n, capacity - at)
        (storage + at).update(from: samples, count: first)
        if n > first { storage.update(from: samples + first, count: n - first) }
        head.store(h + n, ordering: .releasing)
        return n
    }

    /// Producer side for float input (the microphone tap): converts while it copies.
    @discardableResult
    func write(floats: UnsafePointer<Float>, count: Int) -> Int {
        let h = head.load(ordering: .relaxed), t = tail.load(ordering: .acquiring)
        let n = min(count, capacity - (h - t))
        if n < count { dropped.wrappingAdd(count - n, ordering: .relaxed) }
        guard n > 0 else { return 0 }
        var at = h % capacity
        for i in 0..<n {
            let s = floats[i] * 32767
            storage[at] = Int16(s > 32767 ? 32767 : (s < -32768 ? -32768 : s))
            at += 1; if at == capacity { at = 0 }
        }
        head.store(h + n, ordering: .releasing)
        return n
    }

    /// Consumer side. Reads exactly `count` samples, or nothing if fewer are buffered.
    func read(into out: UnsafeMutablePointer<Int16>, count: Int) -> Bool {
        let t = tail.load(ordering: .relaxed), h = head.load(ordering: .acquiring)
        guard h - t >= count else { return false }
        let at = t % capacity, first = min(count, capacity - at)
        out.update(from: storage + at, count: first)
        if count > first { (out + first).update(from: storage, count: count - first) }
        tail.store(t + count, ordering: .releasing)
        return true
    }

    /// Consumer side: forget everything buffered.
    func drain() { tail.store(head.load(ordering: .acquiring), ordering: .releasing) }
}
