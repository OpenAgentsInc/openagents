// Firm reason: 6 (macOS filesystem effects; Coax owns stored shapes and their codecs).
#if os(macOS)
import Foundation
import Darwin

/// Bounded owner-only file effects. No record encoding or account decisions live here.
struct AppPrivateFile {
    let url: URL
    let limit: Int
    private var folder: URL { url.deletingLastPathComponent() }
    private func privateFolder(create: Bool) -> Bool {
        if create { do { try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700]) } catch { return false } }
        var st = stat()
        guard lstat(folder.path, &st) == 0, st.st_mode & S_IFMT == S_IFDIR, st.st_uid == geteuid() else { return false }
        if st.st_mode & 0o077 == 0 { return true }
        // Older desktop installs made this app-owned folder 0755. Tighten it
        // before the first new private write; a read alone changes nothing.
        return create && chmod(folder.path, 0o700) == 0
    }
    private func plainTarget() -> Bool {
        var st = stat()
        if lstat(url.path, &st) != 0 { return errno == ENOENT }
        return st.st_mode & S_IFMT == S_IFREG && st.st_uid == geteuid()
    }
    func load() -> Data? {
        guard limit >= 0, privateFolder(create: false) else { return nil }
        let fd = open(url.path, O_RDONLY | O_NOFOLLOW | O_CLOEXEC)
        guard fd >= 0 else { return nil }; defer { close(fd) }
        var st = stat()
        guard fstat(fd, &st) == 0, st.st_mode & S_IFMT == S_IFREG, st.st_uid == geteuid(), st.st_mode & 0o077 == 0, st.st_size >= 0, st.st_size <= Int64(limit) else { return nil }
        var data = Data(), bytes = [UInt8](repeating: 0, count: 4096)
        while true {
            let n = Darwin.read(fd, &bytes, bytes.count)
            if n == 0 { return data }
            if n < 0 { if errno == EINTR { continue }; return nil }
            guard n <= limit - data.count else { return nil }
            data.append(contentsOf: bytes.prefix(n))
        }
    }
    func replace(_ bytes: Data) -> Bool {
        guard bytes.count <= limit, privateFolder(create: true), plainTarget() else { return false }
        do { try bytes.write(to: url, options: .atomic); return chmod(url.path, 0o600) == 0 } catch { return false }
    }
    /// Pending private commands may be created once and never overwritten.
    func create(_ bytes: Data) -> Bool {
        guard bytes.count <= limit, privateFolder(create: true) else { return false }
        let fd = open(url.path, O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC, 0o600)
        guard fd >= 0 else { return false }
        var complete = false
        defer { close(fd); if !complete { unlink(url.path) } }
        guard fchmod(fd, 0o600) == 0 else { return false }
        let written = bytes.withUnsafeBytes { raw -> Bool in
            var at = 0
            while at < raw.count {
                let n = Darwin.write(fd, raw.baseAddress!.advanced(by: at), raw.count - at)
                if n < 0 && errno == EINTR { continue }
                if n <= 0 { return false }; at += n
            }
            return true
        }
        guard written, fsync(fd) == 0 else { return false }
        let parent = open(folder.path, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC)
        guard parent >= 0 else { return false }; defer { close(parent) }
        guard fsync(parent) == 0 else { return false }
        complete = true; return true
    }
    func remove() { guard privateFolder(create: false), plainTarget() else { return }; _ = unlink(url.path) }
}

struct AppViewState: Equatable {
    var email = ""
    var rejoin = ""
    var environment = ""
}
@MainActor protocol AppViewStore: AnyObject {
    func load() -> AppViewState
    func save(_ state: AppViewState) -> Bool
}
@MainActor final class AppFileViewStore: AppViewStore {
    private let file: AppPrivateFile
    static var folder: URL { FileTokenStore(app: "desktop").url.deletingLastPathComponent() }
    init(url: URL? = nil) { file = AppPrivateFile(url: url ?? AppFileViewStore.folder.appendingPathComponent("native-view-v1.json"), limit: Int(oa_view_capacity())) }
    func load() -> AppViewState {
        guard let raw = file.load() else { return AppViewState() }
        func field(_ which: Int32) -> String? {
            var out = [CChar](repeating: 0, count: Int(oa_view_value_capacity()))
            let n = raw.withUnsafeBytes { p in oa_view_read(p.bindMemory(to: UInt8.self).baseAddress, Int32(raw.count), which, &out, Int32(out.count)) }
            return n >= 0 ? String(cString: out) : nil
        }
        guard let email = field(Int32(OA_VIEW_EMAIL)), let rejoin = field(Int32(OA_VIEW_REJOIN)), let environment = field(Int32(OA_VIEW_ENVIRONMENT)) else { return AppViewState() }
        return AppViewState(email: email, rejoin: rejoin, environment: environment)
    }
    func save(_ state: AppViewState) -> Bool {
        var out = [UInt8](repeating: 0, count: Int(oa_view_capacity()))
        let n = oa_view_write(state.email, state.rejoin, state.environment, &out, Int32(out.count))
        return n > 0 && file.replace(Data(out.prefix(Int(n))))
    }
}

/// Exact already-encoded private commands. Account names become safe opaque
/// filenames through Foundation's base64 encoder, never path components as given.
@MainActor final class AppFileIdentityPending: AppIdentityPendingStore {
    private let folder: URL
    init(folder: URL? = nil) { self.folder = folder ?? AppFileViewStore.folder.appendingPathComponent("pending-identity-v1", isDirectory: true) }
    private func file(_ account: String) -> AppPrivateFile {
        let name = Data(account.utf8).base64EncodedString().replacingOccurrences(of: "/", with: "_").replacingOccurrences(of: "+", with: "-")
        return AppPrivateFile(url: folder.appendingPathComponent(name + ".json"), limit: 1024)
    }
    func load(_ account: String) -> Data? { file(account).load() }
    func save(_ bytes: Data, account: String) -> Bool { file(account).create(bytes) }
    func remove(_ account: String) { file(account).remove() }
}
#endif
