import Foundation
import SQLite3

// SQLite transient destructor helper for bind APIs (forces SQLite to copy data)
private let SQLITE_TRANSIENT = unsafeBitCast(-1, to: sqlite3_destructor_type.self)

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

        CREATE TABLE IF NOT EXISTS orchestration_configs (
          id TEXT NOT NULL,
          workspace_root TEXT NOT NULL,
          json TEXT NOT NULL,
          updated_at INTEGER NOT NULL,
          PRIMARY KEY (id, workspace_root)
        );
        CREATE INDEX IF NOT EXISTS idx_orchestration_configs_workspace ON orchestration_configs(workspace_root);
        CREATE INDEX IF NOT EXISTS idx_orchestration_configs_updated_at ON orchestration_configs(updated_at);
        
        -- Conversation titles (optional user-provided display names)
        CREATE TABLE IF NOT EXISTS conversation_titles (
          session_id TEXT PRIMARY KEY,
          title TEXT NOT NULL,
          updated_at INTEGER NOT NULL
        );
        """
        try exec(create)

        // Embeddings schema (vector storage)
        try ensureVectorSchemaV2()
    }

    private func exec(_ sql: String) throws {
        var err: UnsafeMutablePointer<Int8>? = nil
        if sqlite3_exec(db, sql, nil, nil, &err) != SQLITE_OK {
            let msg = err.flatMap { String(cString: $0) } ?? "unknown"
            sqlite3_free(err)
            throw DbError.execFailed(msg)
        }
    }

    // MARK: - General SQL Helpers

    /// Execute SQL with parameters (INSERT, UPDATE, DELETE)
    public func execute(_ sql: String, params: [Any]) throws {
        var stmt: OpaquePointer?
        guard sqlite3_prepare_v2(db, sql, -1, &stmt, nil) == SQLITE_OK else {
            throw DbError.execFailed("prepare failed: \(sql)")
        }
        defer { sqlite3_finalize(stmt) }

        // Bind parameters
        for (i, param) in params.enumerated() {
            let idx = Int32(i + 1)
            if let s = param as? String {
                sqlite3_bind_text(stmt, idx, (s as NSString).utf8String, -1, nil)
            } else if let data = param as? Data {
                data.withUnsafeBytes { (bytes: UnsafeRawBufferPointer) in
                    let ptr = bytes.bindMemory(to: UInt8.self).baseAddress
                    sqlite3_bind_blob(stmt, idx, ptr, Int32(data.count), SQLITE_TRANSIENT)
                }
            } else if let n = param as? Int {
                sqlite3_bind_int64(stmt, idx, Int64(n))
            } else if let n = param as? Int64 {
                sqlite3_bind_int64(stmt, idx, n)
            } else if let d = param as? Double {
                sqlite3_bind_double(stmt, idx, d)
            } else if param is NSNull {
                sqlite3_bind_null(stmt, idx)
            } else {
                // Treat as NULL for unsupported types
                sqlite3_bind_null(stmt, idx)
            }
        }

        guard sqlite3_step(stmt) == SQLITE_DONE else {
            throw DbError.execFailed("execute step failed")
        }
    }

    /// Query one row returning column_name -> value dictionary
    public func queryOne(_ sql: String, params: [Any]) throws -> [String: Any]? {
        var stmt: OpaquePointer?
        guard sqlite3_prepare_v2(db, sql, -1, &stmt, nil) == SQLITE_OK else {
            throw DbError.execFailed("prepare query failed")
        }
        defer { sqlite3_finalize(stmt) }

        // Bind parameters
        for (i, param) in params.enumerated() {
            let idx = Int32(i + 1)
            if let s = param as? String {
                sqlite3_bind_text(stmt, idx, (s as NSString).utf8String, -1, nil)
            } else if let n = param as? Int {
                sqlite3_bind_int64(stmt, idx, Int64(n))
            } else if let n = param as? Int64 {
                sqlite3_bind_int64(stmt, idx, n)
            } else if let d = param as? Double {
                sqlite3_bind_double(stmt, idx, d)
            } else {
                sqlite3_bind_null(stmt, idx)
            }
        }

        guard sqlite3_step(stmt) == SQLITE_ROW else {
            return nil  // No row found
        }

        var row: [String: Any] = [:]
        let colCount = sqlite3_column_count(stmt)
        for i in 0..<colCount {
            let colName = String(cString: sqlite3_column_name(stmt, i))
            let colType = sqlite3_column_type(stmt, i)

            switch colType {
            case SQLITE_TEXT:
                row[colName] = String(cString: sqlite3_column_text(stmt, i))
            case SQLITE_INTEGER:
                row[colName] = sqlite3_column_int64(stmt, i)
            case SQLITE_FLOAT:
                row[colName] = sqlite3_column_double(stmt, i)
            case SQLITE_BLOB:
                if let bytes = sqlite3_column_blob(stmt, i) {
                    let length = Int(sqlite3_column_bytes(stmt, i))
                    let data = Data(bytes: bytes, count: length)
                    row[colName] = data
                } else {
                    row[colName] = Data()
                }
            case SQLITE_NULL:
                row[colName] = NSNull()
            default:
                row[colName] = NSNull()
            }
        }

        return row
    }

    /// Query all rows returning array of column_name -> value dictionaries
    public func queryAll(_ sql: String, params: [Any]) throws -> [[String: Any]] {
        var stmt: OpaquePointer?
        guard sqlite3_prepare_v2(db, sql, -1, &stmt, nil) == SQLITE_OK else {
            throw DbError.execFailed("prepare query failed")
        }
        defer { sqlite3_finalize(stmt) }

        // Bind parameters
        for (i, param) in params.enumerated() {
            let idx = Int32(i + 1)
            if let s = param as? String {
                sqlite3_bind_text(stmt, idx, (s as NSString).utf8String, -1, nil)
            } else if let n = param as? Int {
                sqlite3_bind_int64(stmt, idx, Int64(n))
            } else if let n = param as? Int64 {
                sqlite3_bind_int64(stmt, idx, n)
            } else if let d = param as? Double {
                sqlite3_bind_double(stmt, idx, d)
            } else {
                sqlite3_bind_null(stmt, idx)
            }
        }

        var rows: [[String: Any]] = []
        while sqlite3_step(stmt) == SQLITE_ROW {
            var row: [String: Any] = [:]
            let colCount = sqlite3_column_count(stmt)
            for i in 0..<colCount {
                let colName = String(cString: sqlite3_column_name(stmt, i))
                let colType = sqlite3_column_type(stmt, i)

            switch colType {
            case SQLITE_TEXT:
                row[colName] = String(cString: sqlite3_column_text(stmt, i))
            case SQLITE_INTEGER:
                row[colName] = sqlite3_column_int64(stmt, i)
            case SQLITE_FLOAT:
                row[colName] = sqlite3_column_double(stmt, i)
            case SQLITE_BLOB:
                if let bytes = sqlite3_column_blob(stmt, i) {
                    let length = Int(sqlite3_column_bytes(stmt, i))
                    let data = Data(bytes: bytes, count: length)
                    row[colName] = data
                } else {
                    row[colName] = Data()
                }
            case SQLITE_NULL:
                row[colName] = NSNull()
            default:
                row[colName] = NSNull()
            }
        }
        rows.append(row)
        }

        return rows
    }

    public struct EventRow: Codable { public let event_id: Int64; public let session_id: String; public let seq: Int64; public let ts: Int64; public let update_json: String; public let meta_json: String? }

    public struct RecentSessionRow: Codable {
        public let session_id: String
        public let last_ts: Int64
        public let message_count: Int64
        public let mode: String?
    }

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
            let meta: String? = metaPtr.flatMap { String(cString: $0) }
            out.append(EventRow(event_id: event_id, session_id: sess, seq: seq, ts: ts, update_json: upd, meta_json: meta))
        }
        return out
    }

    public func recentSessions(limit: Int = 10) throws -> [RecentSessionRow] {
        let sql = """
        SELECT
          main.session_id,
          main.last_ts,
          main.cnt,
          (SELECT json_extract(update_json, '$.current_mode_id')
           FROM acp_events
           WHERE session_id = main.session_id
             AND update_json LIKE '%"sessionUpdate":"current_mode_update"%'
           ORDER BY seq DESC
           LIMIT 1) AS mode
        FROM (
          SELECT session_id, MAX(ts) AS last_ts, COUNT(*) AS cnt
          FROM acp_events
          GROUP BY session_id
        ) AS main
        ORDER BY main.last_ts DESC
        LIMIT ?;
        """
        var stmt: OpaquePointer?
        guard sqlite3_prepare_v2(db, sql, -1, &stmt, nil) == SQLITE_OK else { throw DbError.execFailed("prepare recentSessions") }
        defer { sqlite3_finalize(stmt) }
        sqlite3_bind_int64(stmt, 1, Int64(limit))

        var out: [RecentSessionRow] = []
        while sqlite3_step(stmt) == SQLITE_ROW {
            let session_id = String(cString: sqlite3_column_text(stmt, 0))
            let last_ts = sqlite3_column_int64(stmt, 1)
            let cnt = sqlite3_column_int64(stmt, 2)
            let mode: String? = if sqlite3_column_type(stmt, 3) != SQLITE_NULL {
                String(cString: sqlite3_column_text(stmt, 3))
            } else {
                nil
            }
            out.append(RecentSessionRow(session_id: session_id, last_ts: last_ts, message_count: cnt, mode: mode))
        }
        return out
    }

    public func sessionTimeline(sessionId: String, limit: Int? = nil) throws -> [String] {
        let sql: String
        if let lim = limit {
            sql = "SELECT update_json FROM acp_events WHERE session_id = ? ORDER BY ts ASC LIMIT \(lim);"
        } else {
            sql = "SELECT update_json FROM acp_events WHERE session_id = ? ORDER BY ts ASC;"
        }

        var stmt: OpaquePointer?
        guard sqlite3_prepare_v2(db, sql, -1, &stmt, nil) == SQLITE_OK else { throw DbError.execFailed("prepare sessionTimeline") }
        defer { sqlite3_finalize(stmt) }
        sqlite3_bind_text(stmt, 1, (sessionId as NSString).utf8String, -1, nil)

        var out: [String] = []
        while sqlite3_step(stmt) == SQLITE_ROW {
            let updateJson = String(cString: sqlite3_column_text(stmt, 0))
            out.append(updateJson)
        }
        return out
    }

    // MARK: - Conversation Titles

    public func setSessionTitle(sessionId: String, title: String, updatedAt: Int64) throws {
        let sql = """
        INSERT INTO conversation_titles (session_id, title, updated_at)
        VALUES (?, ?, ?)
        ON CONFLICT(session_id) DO UPDATE SET
          title = excluded.title,
          updated_at = excluded.updated_at;
        """
        try execute(sql, params: [sessionId, title, updatedAt])
    }

    public func getSessionTitle(sessionId: String) throws -> String? {
        let sql = "SELECT title FROM conversation_titles WHERE session_id = ? LIMIT 1;"
        if let row = try queryOne(sql, params: [sessionId]) {
            return row["title"] as? String
        }
        return nil
    }

    public func clearSessionTitle(sessionId: String) throws {
        let sql = "DELETE FROM conversation_titles WHERE session_id = ?;"
        try execute(sql, params: [sessionId])
    }

    // MARK: - Orchestration Config CRUD

    /// Insert or update an orchestration configuration
    public func insertOrUpdateOrchestrationConfig(_ configJSON: String, id: String, workspaceRoot: String, updatedAt: Int64) throws {
        let sql = """
        INSERT INTO orchestration_configs (id, workspace_root, json, updated_at)
        VALUES (?, ?, ?, ?)
        ON CONFLICT(id, workspace_root) DO UPDATE SET
          json = excluded.json,
          updated_at = excluded.updated_at;
        """

        try execute(sql, params: [id, workspaceRoot, configJSON, updatedAt])
    }

    /// Get an orchestration configuration by id and workspace root
    public func getOrchestrationConfig(id: String, workspaceRoot: String) throws -> String? {
        let sql = "SELECT json FROM orchestration_configs WHERE id = ? AND workspace_root = ? LIMIT 1;"

        guard let row = try queryOne(sql, params: [id, workspaceRoot]),
              let json = row["json"] as? String else {
            return nil
        }

        return json
    }

    /// List all orchestration configurations for a workspace
    public func listOrchestrationConfigs(workspaceRoot: String) throws -> [String] {
        let sql = "SELECT json FROM orchestration_configs WHERE workspace_root = ? ORDER BY updated_at DESC;"

        let rows = try queryAll(sql, params: [workspaceRoot])
        return rows.compactMap { $0["json"] as? String }
    }

    /// List all orchestration configurations (all workspaces)
    public func listAllOrchestrationConfigs() throws -> [String] {
        let sql = "SELECT json FROM orchestration_configs ORDER BY workspace_root, updated_at DESC;"

        let rows = try queryAll(sql, params: [])
        return rows.compactMap { $0["json"] as? String }
    }

    /// Delete an orchestration configuration
    public func deleteOrchestrationConfig(id: String, workspaceRoot: String) throws {
        let sql = "DELETE FROM orchestration_configs WHERE id = ? AND workspace_root = ?;"
        try execute(sql, params: [id, workspaceRoot])
    }

    /// Delete all orchestration configurations (use with caution!)
    public func deleteAllOrchestrationConfigs() throws {
        let sql = "DELETE FROM orchestration_configs;"
        try execute(sql, params: [])
    }

    // MARK: - Embeddings Schema & Helpers

    /// Ensure the embeddings table and indexes exist
    private func ensureVectorSchemaV2() throws {
        let createEmbeddingsTable = """
        CREATE TABLE IF NOT EXISTS embeddings (
            id TEXT NOT NULL,
            collection TEXT NOT NULL,
            embedding_blob BLOB NOT NULL,
            dimensions INTEGER NOT NULL,
            model_id TEXT NOT NULL,
            metadata_json TEXT NULL,
            text TEXT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            PRIMARY KEY (id, collection)
        );
        """

        let createIndexes = """
        CREATE INDEX IF NOT EXISTS idx_embeddings_collection ON embeddings(collection);
        CREATE INDEX IF NOT EXISTS idx_embeddings_model ON embeddings(model_id);
        CREATE INDEX IF NOT EXISTS idx_embeddings_updated ON embeddings(updated_at);
        """

        try exec(createEmbeddingsTable)
        try exec(createIndexes)
    }

    /// Store or replace an embedding row
    public func storeEmbedding(
        id: String,
        collection: String,
        embedding: [Float],
        dimensions: Int,
        modelID: String,
        metadata: [String: String]?,
        text: String?
    ) throws {
        let metadataJSON: String? = metadata.flatMap { dict in
            (try? JSONSerialization.data(withJSONObject: dict)).flatMap { String(data: $0, encoding: .utf8) }
        }

        // Serialize Float array to Data
        let embeddingData = embedding.withUnsafeBytes { Data($0) }

        let now = Int64(Date().timeIntervalSince1970 * 1000)

        let sql = """
        INSERT OR REPLACE INTO embeddings
        (id, collection, embedding_blob, dimensions, model_id, metadata_json, text, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        """

        try execute(sql, params: [
            id,
            collection,
            embeddingData,
            dimensions,
            modelID,
            metadataJSON ?? NSNull(),
            text ?? NSNull(),
            now,
            now
        ])
    }

    /// Fetch all embeddings for a collection
    public func fetchEmbeddings(collection: String) throws -> [(id: String, embedding: [Float], dimensions: Int, metadata: [String: String]?)] {
        let sql = "SELECT id, embedding_blob, dimensions, metadata_json FROM embeddings WHERE collection = ?"
        let rows = try queryAll(sql, params: [collection])

        return rows.compactMap { row in
            guard let id = row["id"] as? String,
                  let embeddingData = row["embedding_blob"] as? Data,
                  let dimsAny = row["dimensions"],
                  let dimensions = (dimsAny as? Int64).map(Int.init) ?? (dimsAny as? Int)
            else { return nil }

            let vec: [Float] = embeddingData.withUnsafeBytes {
                let count = embeddingData.count / MemoryLayout<Float>.size
                if count == 0 { return [] }
                let ptr = $0.bindMemory(to: Float.self).baseAddress!
                return Array(UnsafeBufferPointer(start: ptr, count: count))
            }

            let metadata: [String: String]? = (row["metadata_json"] as? String)
                .flatMap { $0.data(using: .utf8) }
                .flatMap { try? JSONSerialization.jsonObject(with: $0) as? [String: String] }

            return (id: id, embedding: vec, dimensions: dimensions, metadata: metadata)
        }
    }
}
