import Foundation
import SQLite3

// MARK: - Tinyvex DbLayer (SQLite3)

public actor TinyvexDbLayer {
    public enum DbError: Error { case openFailed; case execFailed(String) }

    private var db: OpaquePointer?
    private let path: String

    public init(path: String) throws {
        self.path = path
        if sqlite3_open(path, &db) != SQLITE_OK { throw DbError.openFailed }
        try self.applyPragmas()
        try self.migrate()
    }

    deinit { if let db { sqlite3_close(db) } }

    private func applyPragmas() throws {
        try exec("PRAGMA journal_mode=WAL;")
        try exec("PRAGMA synchronous=NORMAL;")
        try exec("PRAGMA busy_timeout=5000;")
    }

    private func migrate() throws {
        let create = """
        CREATE TABLE IF NOT EXISTS acp_events (
          event_id INTEGER PRIMARY KEY AUTOINCREMENT,
          session_id TEXT NOT NULL,
          seq INTEGER NOT NULL,
          ts INTEGER NOT NULL,
          update_json TEXT NOT NULL,
          meta_json TEXT NULL,
          index_hints TEXT NULL
        );
        CREATE INDEX IF NOT EXISTS acp_events_by_session_seq ON acp_events (session_id, seq);
        CREATE INDEX IF NOT EXISTS acp_events_by_ts ON acp_events (ts);
        """
        try exec(create)
    }

    private func exec(_ sql: String) throws {
        var err: UnsafeMutablePointer<Int8>? = nil
        if sqlite3_exec(db, sql, nil, nil, &err) != SQLITE_OK {
            let msg = err.flatMap { String(cString: $0) } ?? "unknown"
            sqlite3_free(err)
            throw DbError.execFailed(msg)
        }
    }

    public struct EventRow: Codable { public let event_id: Int64; public let session_id: String; public let seq: Int64; public let ts: Int64; public let update_json: String; public let meta_json: String? }

    public func appendEvent(sessionId: String, seq: Int64, ts: Int64, updateJSON: String, metaJSON: String? = nil) throws {
        let sql = "INSERT INTO acp_events (session_id, seq, ts, update_json, meta_json) VALUES (?,?,?,?,?);"
        var stmt: OpaquePointer?
        guard sqlite3_prepare_v2(db, sql, -1, &stmt, nil) == SQLITE_OK else { throw DbError.execFailed("prepare insert") }
        defer { sqlite3_finalize(stmt) }
        sqlite3_bind_text(stmt, 1, (sessionId as NSString).utf8String, -1, nil)
        sqlite3_bind_int64(stmt, 2, seq)
        sqlite3_bind_int64(stmt, 3, ts)
        sqlite3_bind_text(stmt, 4, (updateJSON as NSString).utf8String, -1, nil)
        if let metaJSON { sqlite3_bind_text(stmt, 5, (metaJSON as NSString).utf8String, -1, nil) } else { sqlite3_bind_null(stmt, 5) }
        if sqlite3_step(stmt) != SQLITE_DONE { throw DbError.execFailed("insert failed") }
    }

    public func history(sessionId: String, sinceSeq: Int64?, sinceTs: Int64?, limit: Int?) throws -> [EventRow] {
        var clauses: [String] = ["session_id = ?"]
        var binds: [Any] = [sessionId]
        if let s = sinceSeq { clauses.append("seq > ?"); binds.append(s) }
        if let t = sinceTs { clauses.append("ts > ?"); binds.append(t) }
        let whereSQL = clauses.joined(separator: " AND ")
        let lim = limit ?? 100
        let sql = "SELECT event_id, session_id, seq, ts, update_json, meta_json FROM acp_events WHERE \(whereSQL) ORDER BY seq ASC LIMIT \(lim);"
        var stmt: OpaquePointer?
        guard sqlite3_prepare_v2(db, sql, -1, &stmt, nil) == SQLITE_OK else { throw DbError.execFailed("prepare select") }
        defer { sqlite3_finalize(stmt) }
        // Bind
        for (i, v) in binds.enumerated() {
            let idx = Int32(i + 1)
            if let s = v as? String { sqlite3_bind_text(stmt, idx, (s as NSString).utf8String, -1, nil) }
            else if let n = v as? Int64 { sqlite3_bind_int64(stmt, idx, n) }
        }
        var out: [EventRow] = []
        while sqlite3_step(stmt) == SQLITE_ROW {
            let event_id = sqlite3_column_int64(stmt, 0)
            let sess = String(cString: sqlite3_column_text(stmt, 1))
            let seq = sqlite3_column_int64(stmt, 2)
            let ts = sqlite3_column_int64(stmt, 3)
            let upd = String(cString: sqlite3_column_text(stmt, 4))
            let metaPtr = sqlite3_column_text(stmt, 5)
            let meta = metaPtr != nil ? String(cString: metaPtr!) : nil
            out.append(EventRow(event_id: event_id, session_id: sess, seq: seq, ts: ts, update_json: upd, meta_json: meta))
        }
        return out
    }
}
