import Foundation
import Combine
import OpenAgentsCore
#if os(macOS)
import SQLite3
import SwiftUI

final class TinyvexManager: ObservableObject {
    @Published private(set) var isRunning: Bool = false
    @Published private(set) var dbPath: String = ""
    @Published private(set) var tableCount: Int = 0
    @Published private(set) var rowCount: Int64 = 0
    @Published private(set) var fileSizeBytes: Int64 = 0

    // We currently do not bind Tinyvex to the network; DesktopWebSocketServer serves clients.
    // Tinyvex provides persistence (DB) and status only.
    private var server: TinyvexServer?
    private var statusTimer: Timer?
    private var vnodeSource: DispatchSourceFileSystemObject?

    func start() {
        guard !isRunning else { return }
        do {
            let path = TinyvexManager.defaultDbPath()
            dbPath = path.path
            // Initialize DB file (create tables) without starting a WS listener.
            _ = try TinyvexDbLayer(path: dbPath)
            self.isRunning = true
            refreshStatus()
            statusTimer = Timer.scheduledTimer(withTimeInterval: 1.0, repeats: true) { [weak self] _ in self?.refreshStatus() }
            // File change notifications for faster UI refresh
            let fd = open(dbPath, O_EVTONLY)
            if fd >= 0 {
                let src = DispatchSource.makeFileSystemObjectSource(fileDescriptor: fd, eventMask: [.write, .extend, .attrib], queue: .main)
                src.setEventHandler { [weak self] in self?.refreshStatus() }
                src.setCancelHandler { close(fd) }
                src.resume()
                vnodeSource = src
            }
        } catch {
            print("[Tinyvex] Failed to start: \(error)")
        }
    }

    func stop() {
        server?.stop(); server = nil
        statusTimer?.invalidate(); statusTimer = nil
        vnodeSource?.cancel(); vnodeSource = nil
        isRunning = false
    }

    private func refreshStatus() {
        // File size
        let fm = FileManager.default
        if let attrs = try? fm.attributesOfItem(atPath: dbPath), let sz = attrs[.size] as? NSNumber { self.fileSizeBytes = sz.int64Value }
        // Table and row counts
        var db: OpaquePointer?
        if sqlite3_open_v2(dbPath, &db, SQLITE_OPEN_READONLY, nil) == SQLITE_OK, let db {
            defer { sqlite3_close(db) }
            // Tables
            var countStmt: OpaquePointer?
            if sqlite3_prepare_v2(db, "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%';", -1, &countStmt, nil) == SQLITE_OK {
                if sqlite3_step(countStmt) == SQLITE_ROW { self.tableCount = Int(sqlite3_column_int64(countStmt, 0)) }
                sqlite3_finalize(countStmt)
            }
            // Rows total (sum of all user tables)
            var namesStmt: OpaquePointer?
            var total: Int64 = 0
            if sqlite3_prepare_v2(db, "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%';", -1, &namesStmt, nil) == SQLITE_OK {
                while sqlite3_step(namesStmt) == SQLITE_ROW {
                    if let cstr = sqlite3_column_text(namesStmt, 0) {
                        let name = String(cString: cstr)
                        var stmt: OpaquePointer?
                        let q = "SELECT COUNT(*) FROM \(name);"
                        if sqlite3_prepare_v2(db, q, -1, &stmt, nil) == SQLITE_OK {
                            if sqlite3_step(stmt) == SQLITE_ROW { total += sqlite3_column_int64(stmt, 0) }
                        }
                        sqlite3_finalize(stmt)
                    }
                }
                sqlite3_finalize(namesStmt)
            }
            self.rowCount = total
        }
        DispatchQueue.main.async { self.objectWillChange.send() }
    }

    static func defaultDbPath() -> URL {
        let fm = FileManager.default
        #if os(macOS)
        let base = fm.urls(for: .applicationSupportDirectory, in: .userDomainMask).first!
        let dir = base.appendingPathComponent("OpenAgents", isDirectory: true)
        try? fm.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir.appendingPathComponent("tinyvex.sqlite")
        #else
        return fm.temporaryDirectory.appendingPathComponent("tinyvex.sqlite")
        #endif
    }
}

#endif
