// The room's socket (docs/desktop/coax-desktop.md, "Agents that talk to each other"): the app listens on a
// Unix socket; each Codex session's MCP server (the app run with --mcp-room) connects and sends one JSON line
// per room tool call, and reads one line back. Only bytes move here; the desk (desk.coax) does the rest.
import Foundation

enum RoomSocket {
  static func address(_ path: String) -> sockaddr_un? {
    var a = sockaddr_un(); a.sun_family = sa_family_t(AF_UNIX)
    let bytes = Array(path.utf8)
    guard bytes.count < MemoryLayout.size(ofValue: a.sun_path) else { return nil }
    withUnsafeMutableBytes(of: &a.sun_path) { p in for (i, b) in bytes.enumerated() { p[i] = b } }
    return a
  }
  static func connect(_ path: String) -> Int32 {
    guard var a = address(path) else { return -1 }
    let fd = socket(AF_UNIX, SOCK_STREAM, 0)
    if fd < 0 { return -1 }
    let ok = withUnsafePointer(to: &a) { $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { Darwin.connect(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size)) } }
    if ok != 0 { close(fd); return -1 }
    return fd
  }
  // A line from a blocking socket, the rest kept in pending.
  static func readLine(_ fd: Int32, _ pending: inout Data) -> [UInt8]? {
    var buf = [UInt8](repeating: 0, count: 65536)
    while true {
      if let nl = pending.firstIndex(of: 10) {
        let line = Array(pending[pending.startIndex..<nl]); pending = Data(pending[(nl + 1)...]); return line
      }
      let n = read(fd, &buf, buf.count)
      if n <= 0 { return nil }
      pending.append(contentsOf: buf[0..<n])
    }
  }
}

// The app's end: a non-blocking listener, its connections, and their lines, read each frame.
final class RoomServer {
  let path: String
  var listener: Int32 = -1
  var conns: [Int32: Data] = [:]
  init(path: String) {
    self.path = path
    unlink(path)
    guard var a = RoomSocket.address(path) else { return }
    let fd = socket(AF_UNIX, SOCK_STREAM, 0)
    if fd < 0 { return }
    let ok = withUnsafePointer(to: &a) { $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { bind(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size)) } }
    if ok != 0 || listen(fd, 16) != 0 { close(fd); return }
    chmod(path, 0o600)
    _ = fcntl(fd, F_SETFL, fcntl(fd, F_GETFL) | O_NONBLOCK)
    listener = fd
  }
  deinit { close(listener); unlink(path) }
  // New connections and the complete lines that arrived, each handed to line(conn, bytes).
  func poll(_ line: (Int32, [UInt8]) -> Void) {
    if listener < 0 { return }
    while true {
      let c = accept(listener, nil, nil)
      if c < 0 { break }
      _ = fcntl(c, F_SETFL, fcntl(c, F_GETFL) | O_NONBLOCK)
      var one: Int32 = 1
      _ = setsockopt(c, SOL_SOCKET, SO_NOSIGPIPE, &one, socklen_t(MemoryLayout<Int32>.size))
      conns[c] = Data()
    }
    var buf = [UInt8](repeating: 0, count: 65536)
    for (c, _) in conns {
      while true {
        let n = read(c, &buf, buf.count)
        if n > 0 { conns[c]!.append(contentsOf: buf[0..<n]); continue }
        if n == 0 { close(c); conns[c] = nil }
        break
      }
      while let d = conns[c], let nl = d.firstIndex(of: 10) {
        line(c, Array(d[d.startIndex..<nl])); conns[c] = Data(d[(nl + 1)...])
      }
    }
  }
  func send(_ conn: Int32, _ bytes: ArraySlice<UInt8>) {
    guard conns[conn] != nil else { return }
    var b = Array(bytes); b.append(10)
    var at = 0
    while at < b.count {
      let n = b[at...].withUnsafeBytes { write(conn, $0.baseAddress, $0.count) }
      if n <= 0 { if errno == EAGAIN { usleep(1000); continue }; break }
      at += n
    }
  }
}
