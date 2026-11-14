// Firm reason: 6 (Foundation thread/stack and synchronization SDK effects).
import Foundation

// A thread with a deep stack, for the web's character code (Engine.swift Model.deep): iOS gives
// the main thread a megabyte, and that code passes megabytes by value.
final class Deep {
  private var work: (() -> Void)?
  private let go = DispatchSemaphore(value: 0), done = DispatchSemaphore(value: 0)
  init() {
    let t = Thread { [unowned self] in
      // each piece of work in its own pool: what a realm's page lets go of (an inflated pack) goes back at once
      while true { self.go.wait(); autoreleasepool { self.work?() }; self.work = nil; self.done.signal() }
    }
    t.stackSize = 512 << 20
    t.qualityOfService = .userInteractive
    t.name = "everglade.deep"
    t.start()
  }
  func run(_ block: () -> Void) {
    finish()
    withoutActuallyEscaping(block) { b in work = b; go.signal(); done.wait() }
  }
  // Work that takes seconds (a realm's page opening: its pack read, its pictures drawn) is begun
  // here and not waited for, so the screen keeps drawing. `busy` is true until `ended` has seen it
  // end; a `run` meanwhile waits for it first, so whoever must not wait asks `busy` before.
  private(set) var busy = false
  func begin(_ block: @escaping () -> Void) { finish(); busy = true; work = block; go.signal() }
  func ended() -> Bool {
    if busy && done.wait(timeout: .now()) == .success { busy = false }
    return !busy
  }
  private func finish() { if busy { done.wait(); busy = false } }
}

