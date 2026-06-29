// ---------------------------------------------------------------------------
// Bun-native SQLite storage backend for the Breez SDK Spark `Storage` interface
// (#5080).
//
// Pylon runs on Bun. The Breez SDK Spark WASM core loads fine under Bun, but
// its DEFAULT storage backend (`connect()` -> `createDefaultStorage`) requires
// `better-sqlite3`, which Bun does NOT support (oven-sh/bun#4290). The only
// other shipped backends are MySQL/Postgres. So the receive-only backup helper
// could connect under Bun but failed at "initialize database".
//
// Fix (issue #5080 option 1): implement the SDK's `Storage` interface on Bun's
// built-in `bun:sqlite` and inject it via `SdkBuilder.withStorage()`. This is a
// FAITHFUL port of the SDK's reference better-sqlite3 implementation at
//   @breeztech/breez-sdk-spark/nodejs/storage/index.cjs   (1343 lines)
//   @breeztech/breez-sdk-spark/nodejs/storage/migrations.cjs (440 lines)
//   @breeztech/breez-sdk-spark/nodejs/storage/errors.cjs
// for SDK version 0.15.1. The schema, migrations, SQL, row<->object mapping,
// BigInt handling, and method semantics mirror the reference exactly; only the
// better-sqlite3 -> bun:sqlite API differences are adapted:
//
//   - Named binds: better-sqlite3 binds `@name`/`?` object keys directly;
//     bun:sqlite accepts `$`/`:`/`@`-prefixed keys. The SDK's named SQL already
//     uses `@`-prefixed placeholders, so we pass objects keyed with the same
//     `@name` and use positional `?` everywhere the reference does.
//   - `.pluck()`: bun:sqlite has no `.pluck()`. The single pluck site
//     (getPaymentsByParentIds existence probe) is replaced with `.values()` and
//     reads `[0]?.[0]`.
//   - BigInt: `amount`/`fees` are stored as TEXT and read back via `BigInt(...)`
//     exactly like the reference, so no driver-level safe-integers are needed.
//     `timestamp`/`amount_sats`/`vout` are plain JS numbers in both. Revisions
//     are stored as INTEGER but always read via `CAST(... AS TEXT)` and parsed
//     with `BigInt`, matching the reference; we enable `safeIntegers` only for
//     the one `MAX(revision)+1` read that the reference reads as text.
//
// SAFETY: this module is local private wallet state only. It never logs keys,
// seeds, raw Spark addresses/invoices, preimages, or storage paths.
// ---------------------------------------------------------------------------

import { Database } from "bun:sqlite"

/** Mirrors the SDK's StorageError (errors.cjs). */
export class StorageError extends Error {
  cause: unknown
  constructor(message: string, cause: unknown = null) {
    super(message)
    this.name = "StorageError"
    this.cause = cause
  }
}

// Base query for payment lookups. All columns are accessed by name in
// _rowToPayment. parent_payment_id is only used by getPaymentsByParentIds.
// Ported verbatim from index.cjs SELECT_PAYMENT_SQL.
const SELECT_PAYMENT_SQL = `
    SELECT p.id,
           p.payment_type,
           p.status,
           p.amount,
           p.fees,
           p.timestamp,
           p.method,
           p.withdraw_tx_id,
           p.deposit_tx_id,
           p.spark,
           l.invoice AS lightning_invoice,
           l.payment_hash AS lightning_payment_hash,
           l.destination_pubkey AS lightning_destination_pubkey,
           COALESCE(l.description, pm.lnurl_description) AS lightning_description,
           l.preimage AS lightning_preimage,
           l.htlc_status AS lightning_htlc_status,
           l.htlc_expiry_time AS lightning_htlc_expiry_time,
           pm.lnurl_pay_info,
           pm.lnurl_withdraw_info,
           pm.conversion_info,
           pm.conversion_status,
           t.metadata AS token_metadata,
           t.tx_hash AS token_tx_hash,
           t.tx_type AS token_tx_type,
           t.invoice_details AS token_invoice_details,
           s.invoice_details AS spark_invoice_details,
           s.htlc_details AS spark_htlc_details,
           lrm.nostr_zap_request AS lnurl_nostr_zap_request,
           lrm.nostr_zap_receipt AS lnurl_nostr_zap_receipt,
           lrm.sender_comment AS lnurl_sender_comment,
           lrm.payment_hash AS lnurl_payment_hash,
           pm.parent_payment_id
      FROM payments p
      LEFT JOIN payment_details_lightning l ON p.id = l.payment_id
      LEFT JOIN payment_details_token t ON p.id = t.payment_id
      LEFT JOIN payment_details_spark s ON p.id = s.payment_id
      LEFT JOIN payment_metadata pm ON p.id = pm.payment_id
      LEFT JOIN lnurl_receive_metadata lrm ON l.payment_hash = lrm.payment_hash`

// Migrations ported verbatim from migrations.cjs `_getMigrations()`. Order is
// load-bearing: each migration index maps to PRAGMA user_version. Adding,
// removing, or reordering an entry silently corrupts the on-disk schema.
const MIGRATIONS: ReadonlyArray<{ name: string; sql: string | string[] }> = [
  {
    name: "Create initial tables",
    sql: [
      `CREATE TABLE IF NOT EXISTS payments (
                        id TEXT PRIMARY KEY,
                        payment_type TEXT NOT NULL,
                        status TEXT NOT NULL,
                        amount INTEGER NOT NULL,
                        fees INTEGER NOT NULL,
                        timestamp INTEGER NOT NULL,
                        details TEXT,
                        method TEXT
                    )`,
      `CREATE TABLE IF NOT EXISTS settings (
                        key TEXT PRIMARY KEY,
                        value TEXT NOT NULL
                    )`,
      `CREATE INDEX IF NOT EXISTS idx_payments_timestamp ON payments(timestamp DESC)`,
    ],
  },
  {
    name: "Create unclaimed deposits table",
    sql: [
      `CREATE TABLE IF NOT EXISTS unclaimed_deposits (
                        txid TEXT NOT NULL,
                        vout INTEGER NOT NULL,
                        amount_sats INTEGER,
                        claim_error TEXT,
                        refund_tx TEXT,
                        refund_tx_id TEXT,
                        PRIMARY KEY (txid, vout)
                    )`,
      `CREATE INDEX IF NOT EXISTS idx_unclaimed_deposits_txid ON unclaimed_deposits(txid)`,
    ],
  },
  {
    name: "Create payment metadata table",
    sql: [
      `CREATE TABLE IF NOT EXISTS payment_metadata (
                        payment_id TEXT PRIMARY KEY,
                        lnurl_pay_info TEXT,
                        FOREIGN KEY (payment_id) REFERENCES payments(id) ON DELETE CASCADE
                    )`,
      `CREATE INDEX IF NOT EXISTS idx_payment_metadata_payment_id ON payment_metadata(payment_id)`,
    ],
  },
  {
    name: "Add lnurl_description column to payment_metadata",
    sql: `ALTER TABLE payment_metadata ADD COLUMN lnurl_description TEXT`,
  },
  {
    name: "Flatten payment details",
    sql: [
      `ALTER TABLE payments ADD COLUMN withdraw_tx_id TEXT`,
      `ALTER TABLE payments ADD COLUMN deposit_tx_id TEXT`,
      `ALTER TABLE payments ADD COLUMN spark INTEGER`,
      `CREATE TABLE payment_details_lightning (
              payment_id TEXT PRIMARY KEY,
              invoice TEXT NOT NULL,
              payment_hash TEXT NOT NULL,
              destination_pubkey TEXT NOT NULL,
              description TEXT,
              preimage TEXT,
              FOREIGN KEY (payment_id) REFERENCES payments(id) ON DELETE CASCADE
            )`,
      `INSERT INTO payment_details_lightning (payment_id, invoice, payment_hash, destination_pubkey, description, preimage)
            SELECT id, json_extract(details, '$.Lightning.invoice'), json_extract(details, '$.Lightning.payment_hash'),
                json_extract(details, '$.Lightning.destination_pubkey'), json_extract(details, '$.Lightning.description'),
                json_extract(details, '$.Lightning.preimage')
            FROM payments WHERE json_extract(details, '$.Lightning.invoice') IS NOT NULL`,
      `UPDATE payments SET withdraw_tx_id = json_extract(details, '$.Withdraw.tx_id')
            WHERE json_extract(details, '$.Withdraw.tx_id') IS NOT NULL`,
      `UPDATE payments SET deposit_tx_id = json_extract(details, '$.Deposit.tx_id')
            WHERE json_extract(details, '$.Deposit.tx_id') IS NOT NULL`,
      `ALTER TABLE payments DROP COLUMN details`,
      `CREATE INDEX idx_payment_details_lightning_invoice ON payment_details_lightning(invoice)`,
    ],
  },
  {
    name: "Create payment_details_token table",
    sql: [
      `CREATE TABLE IF NOT EXISTS payment_details_token (
              payment_id TEXT PRIMARY KEY,
              metadata TEXT,
              tx_hash TEXT,
              FOREIGN KEY (payment_id) REFERENCES payments(id) ON DELETE CASCADE
            )`,
    ],
  },
  {
    name: "Change payments amount and fees from INTEGER to TEXT",
    sql: [
      `CREATE TABLE payments_new (
                        id TEXT PRIMARY KEY,
                        payment_type TEXT NOT NULL,
                        status TEXT NOT NULL,
                        amount TEXT NOT NULL,
                        fees TEXT NOT NULL,
                        timestamp INTEGER NOT NULL,
                        method TEXT,
                        withdraw_tx_id TEXT,
                        deposit_tx_id TEXT,
                        spark INTEGER
                    )`,
      `INSERT INTO payments_new (id, payment_type, status, amount, fees, timestamp, method, withdraw_tx_id, deposit_tx_id, spark)
           SELECT id, payment_type, status, CAST(amount AS TEXT), CAST(fees AS TEXT), timestamp, method, withdraw_tx_id, deposit_tx_id, spark
           FROM payments`,
      `DROP TABLE payments`,
      `ALTER TABLE payments_new RENAME TO payments`,
      `CREATE INDEX IF NOT EXISTS idx_payments_timestamp ON payments(timestamp DESC)`,
    ],
  },
  {
    name: "Add spark invoice details",
    sql: [
      `CREATE TABLE payment_details_spark (
              payment_id TEXT NOT NULL PRIMARY KEY,
              invoice_details TEXT NOT NULL,
              FOREIGN KEY (payment_id) REFERENCES payments(id) ON DELETE CASCADE
            )`,
      `ALTER TABLE payment_details_token ADD COLUMN invoice_details TEXT`,
    ],
  },
  {
    name: "Add lnurl_withdraw_info column to payment_metadata",
    sql: `ALTER TABLE payment_metadata ADD COLUMN lnurl_withdraw_info TEXT`,
  },
  {
    name: "Create sync tables",
    sql: [
      `CREATE TABLE sync_revision (
            revision INTEGER NOT NULL DEFAULT 0
          )`,
      `INSERT INTO sync_revision (revision) VALUES (0)`,
      `CREATE TABLE sync_outgoing (
            record_type TEXT NOT NULL,
            data_id TEXT NOT NULL,
            schema_version TEXT NOT NULL,
            commit_time INTEGER NOT NULL,
            updated_fields_json TEXT NOT NULL,
            revision INTEGER NOT NULL
          )`,
      `CREATE INDEX idx_sync_outgoing_data_id_record_type ON sync_outgoing(record_type, data_id)`,
      `CREATE TABLE sync_state (
            record_type TEXT NOT NULL,
            data_id TEXT NOT NULL,
            schema_version TEXT NOT NULL,
            commit_time INTEGER NOT NULL,
            data TEXT NOT NULL,
            revision INTEGER NOT NULL,
            PRIMARY KEY (record_type, data_id)
          )`,
      `CREATE TABLE sync_incoming (
            record_type TEXT NOT NULL,
            data_id TEXT NOT NULL,
            schema_version TEXT NOT NULL,
            commit_time INTEGER NOT NULL,
            data TEXT NOT NULL,
            revision INTEGER NOT NULL,
            PRIMARY KEY (record_type, data_id, revision)
          )`,
      `CREATE INDEX idx_sync_incoming_revision ON sync_incoming(revision)`,
    ],
  },
  {
    name: "Add htlc details to payment_details_spark",
    sql: [
      `ALTER TABLE payment_details_spark RENAME TO tmp_payment_details_spark`,
      `CREATE TABLE payment_details_spark (
            payment_id TEXT NOT NULL PRIMARY KEY,
            invoice_details TEXT,
            htlc_details TEXT,
            FOREIGN KEY (payment_id) REFERENCES payments(id) ON DELETE CASCADE
          )`,
      `INSERT INTO payment_details_spark (payment_id, invoice_details)
            SELECT payment_id, invoice_details FROM tmp_payment_details_spark`,
      `DROP TABLE tmp_payment_details_spark`,
    ],
  },
  {
    name: "Create lnurl_receive_metadata table",
    sql: `CREATE TABLE lnurl_receive_metadata (
                payment_hash TEXT NOT NULL PRIMARY KEY,
                nostr_zap_request TEXT,
                nostr_zap_receipt TEXT,
                sender_comment TEXT
            )`,
  },
  {
    name: "Clear unclaimed deposits for claim_error format change",
    sql: `DELETE FROM unclaimed_deposits`,
  },
  {
    name: "Clear sync tables for BreezSigner backward compatibility",
    sql: [
      `DELETE FROM sync_outgoing`,
      `DELETE FROM sync_incoming`,
      `DELETE FROM sync_state`,
      `UPDATE sync_revision SET revision = 0`,
      `DELETE FROM settings WHERE key = 'sync_initial_complete'`,
    ],
  },
  {
    name: "Add token conversion info to payment_metadata",
    sql: `ALTER TABLE payment_metadata ADD COLUMN token_conversion_info TEXT`,
  },
  {
    name: "Add parent payment id to payment_metadata",
    sql: `ALTER TABLE payment_metadata ADD COLUMN parent_payment_id TEXT`,
  },
  {
    name: "Add conversion info to payment_metadata",
    sql: [
      `ALTER TABLE payment_metadata DROP COLUMN token_conversion_info`,
      `ALTER TABLE payment_metadata ADD COLUMN conversion_info TEXT`,
    ],
  },
  {
    name: "Add tx_type column to payment_details_token",
    sql: [
      `ALTER TABLE payment_details_token ADD COLUMN tx_type TEXT NOT NULL DEFAULT 'transfer'`,
      `UPDATE settings
           SET value = json_set(value, '$.last_synced_final_token_payment_id', NULL)
           WHERE key = 'sync_offset' AND json_valid(value) AND json_type(value, '$.last_synced_final_token_payment_id') IS NOT NULL`,
    ],
  },
  {
    name: "Clear sync tables to force re-sync",
    sql: [
      `DELETE FROM sync_outgoing`,
      `DELETE FROM sync_incoming`,
      `DELETE FROM sync_state`,
      `UPDATE sync_revision SET revision = 0`,
      `DELETE FROM settings WHERE key = 'sync_initial_complete'`,
    ],
  },
  {
    name: "Add preimage column to lnurl_receive_metadata for LUD-21 and NIP-57",
    sql: [
      `ALTER TABLE lnurl_receive_metadata ADD COLUMN preimage TEXT`,
      `DELETE FROM settings WHERE key = 'lnurl_metadata_updated_after'`,
    ],
  },
  {
    name: "Add htlc_status and htlc_expiry_time to lightning payments",
    sql: [
      `ALTER TABLE payment_details_lightning ADD COLUMN htlc_status TEXT NOT NULL DEFAULT 'waitingForPreimage'`,
      `ALTER TABLE payment_details_lightning ADD COLUMN htlc_expiry_time INTEGER NOT NULL DEFAULT 0`,
    ],
  },
  {
    name: "Backfill htlc_status for existing Lightning payments",
    sql: [
      `UPDATE payment_details_lightning
           SET htlc_status = CASE
                   WHEN (SELECT status FROM payments WHERE id = payment_id) = 'completed' THEN 'preimageShared'
                   WHEN (SELECT status FROM payments WHERE id = payment_id) = 'pending' THEN 'waitingForPreimage'
                   ELSE 'returned'
               END`,
      `UPDATE settings
           SET value = json_set(value, '$.offset', 0)
           WHERE key = 'sync_offset' AND json_valid(value)`,
    ],
  },
  {
    name: "Clear cached lightning address for LnurlInfo schema change",
    sql: `DELETE FROM settings WHERE key = 'lightning_address'`,
  },
  {
    name: "Add index on payment_hash for JOIN with lnurl_receive_metadata",
    sql: `CREATE INDEX IF NOT EXISTS idx_payment_details_lightning_payment_hash ON payment_details_lightning(payment_hash)`,
  },
  {
    name: "Create contacts table",
    sql: `CREATE TABLE contacts (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          payment_identifier TEXT NOT NULL,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL
        )`,
  },
  {
    name: "Drop preimage column from lnurl_receive_metadata",
    sql: `ALTER TABLE lnurl_receive_metadata DROP COLUMN preimage`,
  },
  {
    name: "Clear cached lightning address for CachedLightningAddress format change",
    sql: `DELETE FROM settings WHERE key = 'lightning_address'`,
  },
  {
    name: "Add is_mature to unclaimed_deposits",
    sql: [`ALTER TABLE unclaimed_deposits ADD COLUMN is_mature INTEGER NOT NULL DEFAULT 1`],
  },
  {
    name: "Add conversion_status to payment_metadata",
    sql: `ALTER TABLE payment_metadata ADD COLUMN conversion_status TEXT`,
  },
  {
    name: "Drop foreign key on payment_metadata",
    sql: [
      `CREATE TABLE payment_metadata_new (
              payment_id TEXT PRIMARY KEY,
              parent_payment_id TEXT,
              lnurl_pay_info TEXT,
              lnurl_description TEXT,
              lnurl_withdraw_info TEXT,
              conversion_info TEXT,
              conversion_status TEXT
          )`,
      `INSERT INTO payment_metadata_new
              (payment_id, parent_payment_id, lnurl_pay_info, lnurl_description,
               lnurl_withdraw_info, conversion_info, conversion_status)
           SELECT payment_id, parent_payment_id, lnurl_pay_info, lnurl_description,
                  lnurl_withdraw_info, conversion_info, conversion_status
           FROM payment_metadata`,
      `DROP TABLE payment_metadata`,
      `ALTER TABLE payment_metadata_new RENAME TO payment_metadata`,
      `CREATE INDEX idx_payment_metadata_payment_id ON payment_metadata(payment_id)`,
    ],
  },
]

type Row = Record<string, unknown>

/**
 * Bun-native implementation of the Breez SDK Spark `Storage` interface,
 * backed by `bun:sqlite`. Faithful port of the reference better-sqlite3
 * `SqliteStorage` (index.cjs) + `MigrationManager` (migrations.cjs).
 *
 * Pass `":memory:"` for an ephemeral test DB or an absolute file path
 * (e.g. `<dir>/storage.sql`) to match the reference default storage location.
 */
export class SparkBunStorage {
  private db: Database
  private dbPath: string

  constructor(dbPath: string) {
    this.dbPath = dbPath
    try {
      this.db = new Database(dbPath, { create: true })
      // The reference reads revisions back via CAST(... AS TEXT); the one place
      // it reads a raw INTEGER revision (MAX(revision)+1) is also cast to TEXT,
      // so we never need driver-level safe integers for sync. We still leave the
      // default (numbers) for plain INTEGER columns (timestamp, amount_sats,
      // vout) which fit safely in JS numbers, matching the reference.
      this.migrate()
    } catch (error) {
      throw new StorageError(
        `Failed to initialize database at '${this.dbPath}': ${errMessage(error)}`,
        error,
      )
    }
  }

  // ===== Migrations (port of MigrationManager) =====

  private migrate(): void {
    const currentVersion = this.getCurrentVersion()
    const targetVersion = MIGRATIONS.length
    if (currentVersion >= targetVersion) return

    try {
      const run = this.db.transaction(() => {
        for (let i = currentVersion; i < targetVersion; i++) {
          const migration = MIGRATIONS[i]!
          if (Array.isArray(migration.sql)) {
            for (const sql of migration.sql) this.db.exec(sql)
          } else {
            this.db.exec(migration.sql)
          }
        }
        // PRAGMA user_version does not accept a bind param; interpolate the
        // integer (targetVersion is derived from MIGRATIONS.length, not input).
        this.db.exec(`PRAGMA user_version = ${targetVersion}`)
      })
      run()
    } catch (error) {
      throw new StorageError(`Migration failed at version ${currentVersion}: ${errMessage(error)}`, error)
    }
  }

  private getCurrentVersion(): number {
    try {
      const row = this.db.query("PRAGMA user_version").get() as { user_version?: number } | null
      return row?.user_version ?? 0
    } catch {
      return 0
    }
  }

  /** Close the database connection. */
  close(): void {
    this.db.close()
  }

  // ===== Cache Operations =====

  getCachedItem(key: string): Promise<string | null> {
    try {
      const row = this.db.query("SELECT value FROM settings WHERE key = ?").get(key) as
        | { value: string }
        | null
      return Promise.resolve(row ? row.value : null)
    } catch (error) {
      return Promise.reject(new StorageError(`Failed to get cached item '${key}': ${errMessage(error)}`, error))
    }
  }

  setCachedItem(key: string, value: string): Promise<void> {
    try {
      this.db.query("INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)").run(key, value)
      return Promise.resolve()
    } catch (error) {
      return Promise.reject(new StorageError(`Failed to set cached item '${key}': ${errMessage(error)}`, error))
    }
  }

  deleteCachedItem(key: string): Promise<void> {
    try {
      this.db.query("DELETE FROM settings WHERE key = ?").run(key)
      return Promise.resolve()
    } catch (error) {
      return Promise.reject(new StorageError(`Failed to delete cached item '${key}': ${errMessage(error)}`, error))
    }
  }

  // ===== Payment Operations =====

  listPayments(request: any): Promise<any[]> {
    try {
      const actualOffset = request?.offset != null ? request.offset : 0
      const actualLimit = request?.limit != null ? request.limit : 4294967295 // u32::MAX

      const whereClauses: string[] = []
      const params: unknown[] = []

      if (request?.typeFilter && request.typeFilter.length > 0) {
        const placeholders = request.typeFilter.map(() => "?").join(", ")
        whereClauses.push(`p.payment_type IN (${placeholders})`)
        params.push(...request.typeFilter)
      }

      if (request?.statusFilter && request.statusFilter.length > 0) {
        const placeholders = request.statusFilter.map(() => "?").join(", ")
        whereClauses.push(`p.status IN (${placeholders})`)
        params.push(...request.statusFilter)
      }

      if (request?.fromTimestamp != null) {
        whereClauses.push("p.timestamp >= ?")
        params.push(request.fromTimestamp)
      }

      if (request?.toTimestamp != null) {
        whereClauses.push("p.timestamp < ?")
        params.push(request.toTimestamp)
      }

      if (request?.paymentDetailsFilter && request.paymentDetailsFilter.length > 0) {
        const allPaymentDetailsClauses: string[] = []
        for (const paymentDetailsFilter of request.paymentDetailsFilter) {
          const paymentDetailsClauses: string[] = []
          const htlcAlias =
            paymentDetailsFilter.type === "spark"
              ? "s"
              : paymentDetailsFilter.type === "lightning"
                ? "l"
                : null
          if (
            htlcAlias &&
            paymentDetailsFilter.htlcStatus !== undefined &&
            paymentDetailsFilter.htlcStatus.length > 0
          ) {
            const placeholders = paymentDetailsFilter.htlcStatus.map(() => "?").join(", ")
            if (htlcAlias === "l") {
              paymentDetailsClauses.push(`l.htlc_status IN (${placeholders})`)
            } else {
              paymentDetailsClauses.push(`json_extract(s.htlc_details, '$.status') IN (${placeholders})`)
            }
            params.push(...paymentDetailsFilter.htlcStatus)
          }
          if (
            (paymentDetailsFilter.type === "spark" || paymentDetailsFilter.type === "token") &&
            paymentDetailsFilter.conversionRefundNeeded !== undefined
          ) {
            const typeCheck = paymentDetailsFilter.type === "spark" ? "p.spark = 1" : "p.spark IS NULL"
            const refundNeeded =
              paymentDetailsFilter.conversionRefundNeeded === true ? "= 'refundNeeded'" : "!= 'refundNeeded'"
            paymentDetailsClauses.push(
              `${typeCheck} AND pm.conversion_info IS NOT NULL AND
              json_extract(pm.conversion_info, '$.status') ${refundNeeded}`,
            )
          }
          if (paymentDetailsFilter.type === "token" && paymentDetailsFilter.txHash !== undefined) {
            paymentDetailsClauses.push("t.tx_hash = ?")
            params.push(paymentDetailsFilter.txHash)
          }
          if (paymentDetailsFilter.type === "token" && paymentDetailsFilter.txType !== undefined) {
            paymentDetailsClauses.push("t.tx_type = ?")
            params.push(paymentDetailsFilter.txType)
          }
          if (paymentDetailsClauses.length > 0) {
            allPaymentDetailsClauses.push(`(${paymentDetailsClauses.join(" AND ")})`)
          }
        }

        if (allPaymentDetailsClauses.length > 0) {
          whereClauses.push(`(${allPaymentDetailsClauses.join(" OR ")})`)
        }
      }

      if (request?.assetFilter) {
        const assetFilter = request.assetFilter
        if (assetFilter.type === "bitcoin") {
          whereClauses.push("t.metadata IS NULL")
        } else if (assetFilter.type === "token") {
          whereClauses.push("t.metadata IS NOT NULL")
          if (assetFilter.tokenIdentifier) {
            whereClauses.push("json_extract(t.metadata, '$.identifier') = ?")
            params.push(assetFilter.tokenIdentifier)
          }
        }
      }

      // Exclude child payments (those with a parent_payment_id)
      whereClauses.push("pm.parent_payment_id IS NULL")

      const whereSql = whereClauses.length > 0 ? `WHERE ${whereClauses.join(" AND ")}` : ""
      const orderDirection = request?.sortAscending ? "ASC" : "DESC"
      const query = `${SELECT_PAYMENT_SQL} ${whereSql} ORDER BY p.timestamp ${orderDirection} LIMIT ? OFFSET ?`

      params.push(actualLimit, actualOffset)
      const rows = this.db.query(query).all(...(params as any[])) as Row[]
      const payments = rows.map((row) => this._rowToPayment(row))
      return Promise.resolve(payments)
    } catch (error) {
      return Promise.reject(
        new StorageError(
          `Failed to list payments (request: ${JSON.stringify(request)}: ${errMessage(error)}`,
          error,
        ),
      )
    }
  }

  insertPayment(payment: any): Promise<void> {
    try {
      if (!payment) {
        return Promise.reject(new StorageError("Payment cannot be null or undefined"))
      }

      const paymentInsert = this.db.query(
        `INSERT INTO payments (id, payment_type, status, amount, fees, timestamp, method, withdraw_tx_id, deposit_tx_id, spark)
         VALUES (@id, @paymentType, @status, @amount, @fees, @timestamp, @method, @withdrawTxId, @depositTxId, @spark)
         ON CONFLICT(id) DO UPDATE SET
           payment_type=excluded.payment_type,
           status=excluded.status,
           amount=excluded.amount,
           fees=excluded.fees,
           timestamp=excluded.timestamp,
           method=excluded.method,
           withdraw_tx_id=excluded.withdraw_tx_id,
           deposit_tx_id=excluded.deposit_tx_id,
           spark=excluded.spark`,
      )
      const lightningInsert = this.db.query(
        `INSERT INTO payment_details_lightning
          (payment_id, invoice, payment_hash, destination_pubkey, description, preimage, htlc_status, htlc_expiry_time)
          VALUES (@id, @invoice, @paymentHash, @destinationPubkey, @description, @preimage, @htlcStatus, @htlcExpiryTime)
          ON CONFLICT(payment_id) DO UPDATE SET
            invoice=excluded.invoice,
            payment_hash=excluded.payment_hash,
            destination_pubkey=excluded.destination_pubkey,
            description=excluded.description,
            preimage=COALESCE(excluded.preimage, payment_details_lightning.preimage),
            htlc_status=COALESCE(excluded.htlc_status, payment_details_lightning.htlc_status),
            htlc_expiry_time=COALESCE(excluded.htlc_expiry_time, payment_details_lightning.htlc_expiry_time)`,
      )
      const tokenInsert = this.db.query(
        `INSERT INTO payment_details_token
          (payment_id, metadata, tx_hash, tx_type, invoice_details)
          VALUES (@id, @metadata, @txHash, @txType, @invoiceDetails)
          ON CONFLICT(payment_id) DO UPDATE SET
            metadata=excluded.metadata,
            tx_hash=excluded.tx_hash,
            tx_type=excluded.tx_type,
            invoice_details=COALESCE(excluded.invoice_details, payment_details_token.invoice_details)`,
      )
      const sparkInsert = this.db.query(
        `INSERT INTO payment_details_spark
          (payment_id, invoice_details, htlc_details)
          VALUES (@id, @invoiceDetails, @htlcDetails)
          ON CONFLICT(payment_id) DO UPDATE SET
            invoice_details=COALESCE(excluded.invoice_details, payment_details_spark.invoice_details),
            htlc_details=COALESCE(excluded.htlc_details, payment_details_spark.htlc_details)`,
      )
      const transaction = this.db.transaction(() => {
        paymentInsert.run({
          "@id": payment.id,
          "@paymentType": payment.paymentType,
          "@status": payment.status,
          "@amount": payment.amount.toString(),
          "@fees": payment.fees.toString(),
          "@timestamp": payment.timestamp,
          "@method": payment.method ? JSON.stringify(payment.method) : null,
          "@withdrawTxId": payment.details?.type === "withdraw" ? payment.details.txId : null,
          "@depositTxId": payment.details?.type === "deposit" ? payment.details.txId : null,
          "@spark": payment.details?.type === "spark" ? 1 : null,
        })

        if (
          payment.details?.type === "spark" &&
          (payment.details.invoiceDetails != null || payment.details.htlcDetails != null)
        ) {
          sparkInsert.run({
            "@id": payment.id,
            "@invoiceDetails": payment.details.invoiceDetails
              ? JSON.stringify(payment.details.invoiceDetails)
              : null,
            "@htlcDetails": payment.details.htlcDetails ? JSON.stringify(payment.details.htlcDetails) : null,
          })
        }

        if (payment.details?.type === "lightning") {
          lightningInsert.run({
            "@id": payment.id,
            "@invoice": payment.details.invoice,
            "@paymentHash": payment.details.htlcDetails.paymentHash,
            "@destinationPubkey": payment.details.destinationPubkey,
            "@description": payment.details.description ?? null,
            "@preimage": payment.details.htlcDetails?.preimage ?? null,
            "@htlcStatus": payment.details.htlcDetails?.status ?? null,
            "@htlcExpiryTime": payment.details.htlcDetails?.expiryTime ?? 0,
          })
        }

        if (payment.details?.type === "token") {
          tokenInsert.run({
            "@id": payment.id,
            "@metadata": JSON.stringify(payment.details.metadata),
            "@txHash": payment.details.txHash,
            "@txType": payment.details.txType,
            "@invoiceDetails": payment.details.invoiceDetails
              ? JSON.stringify(payment.details.invoiceDetails)
              : null,
          })
        }
      })

      transaction()
      return Promise.resolve()
    } catch (error) {
      return Promise.reject(new StorageError(`Failed to insert payment '${payment?.id}': ${errMessage(error)}`, error))
    }
  }

  getPaymentById(id: string): Promise<any> {
    try {
      if (!id) {
        return Promise.reject(new StorageError("Payment ID cannot be null or undefined"))
      }
      const row = this.db.query(`${SELECT_PAYMENT_SQL} WHERE p.id = ?`).get(id) as Row | null
      if (!row) {
        return Promise.reject(new StorageError(`Payment with id '${id}' not found`))
      }
      return Promise.resolve(this._rowToPayment(row))
    } catch (error) {
      if (error instanceof StorageError) return Promise.reject(error)
      const paymentId = id || "unknown"
      return Promise.reject(new StorageError(`Failed to get payment by id '${paymentId}': ${errMessage(error)}`, error))
    }
  }

  getPaymentByInvoice(invoice: string): Promise<any> {
    try {
      if (!invoice) {
        return Promise.reject(new StorageError("Invoice cannot be null or undefined"))
      }
      const row = this.db.query(`${SELECT_PAYMENT_SQL} WHERE l.invoice = ?`).get(invoice) as Row | null
      if (!row) {
        return Promise.resolve(null)
      }
      return Promise.resolve(this._rowToPayment(row))
    } catch (error) {
      if (error instanceof StorageError) return Promise.reject(error)
      return Promise.reject(
        new StorageError(`Failed to get payment by invoice '${invoice}': ${errMessage(error)}`, error),
      )
    }
  }

  getPaymentsByParentIds(parentPaymentIds: string[]): Promise<{ [parentId: string]: any[] }> {
    try {
      if (!parentPaymentIds || parentPaymentIds.length === 0) {
        return Promise.resolve({})
      }

      // Early exit if no related payments exist. Reference uses .pluck().get();
      // bun:sqlite has no .pluck(), so read the first column via .values().
      const probe = this.db
        .query("SELECT EXISTS(SELECT 1 FROM payment_metadata WHERE parent_payment_id IS NOT NULL LIMIT 1)")
        .values() as Array<Array<unknown>>
      const hasRelated = probe.length > 0 ? probe[0]![0] : 0
      if (!hasRelated) {
        return Promise.resolve({})
      }

      const placeholders = parentPaymentIds.map(() => "?").join(", ")
      const query = `${SELECT_PAYMENT_SQL} WHERE pm.parent_payment_id IN (${placeholders}) ORDER BY p.timestamp ASC`
      const rows = this.db.query(query).all(...parentPaymentIds) as Row[]

      const result: { [parentId: string]: any[] } = {}
      for (const row of rows) {
        const parentId = row.parent_payment_id as string
        if (!result[parentId]) result[parentId] = []
        result[parentId].push(this._rowToPayment(row))
      }
      return Promise.resolve(result)
    } catch (error) {
      return Promise.reject(new StorageError(`Failed to get payments by parent ids: ${errMessage(error)}`, error))
    }
  }

  insertPaymentMetadata(paymentId: string, metadata: any): Promise<void> {
    try {
      const stmt = this.db.query(`
                INSERT INTO payment_metadata (payment_id, parent_payment_id, lnurl_pay_info, lnurl_withdraw_info, lnurl_description, conversion_info, conversion_status)
                VALUES (?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(payment_id) DO UPDATE SET
                    parent_payment_id = COALESCE(excluded.parent_payment_id, parent_payment_id),
                    lnurl_pay_info = COALESCE(excluded.lnurl_pay_info, lnurl_pay_info),
                    lnurl_withdraw_info = COALESCE(excluded.lnurl_withdraw_info, lnurl_withdraw_info),
                    lnurl_description = COALESCE(excluded.lnurl_description, lnurl_description),
                    conversion_info = COALESCE(excluded.conversion_info, conversion_info),
                    conversion_status = COALESCE(excluded.conversion_status, conversion_status)
            `)
      stmt.run(
        paymentId,
        metadata.parentPaymentId ?? null,
        metadata.lnurlPayInfo ? JSON.stringify(metadata.lnurlPayInfo) : null,
        metadata.lnurlWithdrawInfo ? JSON.stringify(metadata.lnurlWithdrawInfo) : null,
        metadata.lnurlDescription ?? null,
        metadata.conversionInfo ? JSON.stringify(metadata.conversionInfo) : null,
        metadata.conversionStatus ?? null,
      )
      return Promise.resolve()
    } catch (error) {
      return Promise.reject(
        new StorageError(`Failed to set payment metadata for '${paymentId}': ${errMessage(error)}`, error),
      )
    }
  }

  // ===== Deposit Operations =====

  addDeposit(txid: string, vout: number, amountSats: number, isMature: boolean): Promise<void> {
    try {
      this.db
        .query(
          `INSERT INTO unclaimed_deposits (txid, vout, amount_sats, is_mature)
                 VALUES (?, ?, ?, ?)
                 ON CONFLICT(txid, vout) DO UPDATE SET is_mature = excluded.is_mature, amount_sats = excluded.amount_sats`,
        )
        .run(txid, vout, amountSats, isMature ? 1 : 0)
      return Promise.resolve()
    } catch (error) {
      return Promise.reject(new StorageError(`Failed to add deposit '${txid}:${vout}': ${errMessage(error)}`, error))
    }
  }

  deleteDeposit(txid: string, vout: number): Promise<void> {
    try {
      this.db.query(`DELETE FROM unclaimed_deposits WHERE txid = ? AND vout = ?`).run(txid, vout)
      return Promise.resolve()
    } catch (error) {
      return Promise.reject(new StorageError(`Failed to delete deposit '${txid}:${vout}': ${errMessage(error)}`, error))
    }
  }

  listDeposits(): Promise<any[]> {
    try {
      const rows = this.db
        .query(
          `SELECT txid, vout, amount_sats, is_mature, claim_error, refund_tx, refund_tx_id
                FROM unclaimed_deposits`,
        )
        .all() as Row[]
      return Promise.resolve(
        rows.map((row) => ({
          txid: row.txid,
          vout: row.vout,
          amountSats: row.amount_sats,
          isMature: Boolean(row.is_mature ?? 1),
          claimError: row.claim_error ? JSON.parse(row.claim_error as string) : null,
          refundTx: row.refund_tx,
          refundTxId: row.refund_tx_id,
        })),
      )
    } catch (error) {
      return Promise.reject(new StorageError(`Failed to list deposits: ${errMessage(error)}`, error))
    }
  }

  updateDeposit(txid: string, vout: number, payload: any): Promise<void> {
    try {
      if (payload.type === "claimError") {
        this.db
          .query(
            `UPDATE unclaimed_deposits
          SET claim_error = ?, refund_tx = NULL, refund_tx_id = NULL
          WHERE txid = ? AND vout = ?`,
          )
          .run(JSON.stringify(payload.error), txid, vout)
      } else if (payload.type === "refund") {
        this.db
          .query(
            `UPDATE unclaimed_deposits
          SET refund_tx = ?, refund_tx_id = ?, claim_error = NULL
          WHERE txid = ? AND vout = ?`,
          )
          .run(payload.refundTx, payload.refundTxid, txid, vout)
      } else {
        return Promise.reject(new StorageError(`Unknown payload type: ${payload.type}`))
      }
      return Promise.resolve()
    } catch (error) {
      if (error instanceof StorageError) return Promise.reject(error)
      return Promise.reject(new StorageError(`Failed to update deposit '${txid}:${vout}': ${errMessage(error)}`, error))
    }
  }

  setLnurlMetadata(metadata: any[]): Promise<void> {
    try {
      const stmt = this.db.query(
        "INSERT OR REPLACE INTO lnurl_receive_metadata (payment_hash, nostr_zap_request, nostr_zap_receipt, sender_comment) VALUES (?, ?, ?, ?)",
      )
      const transaction = this.db.transaction(() => {
        for (const item of metadata) {
          stmt.run(
            item.paymentHash,
            item.nostrZapRequest || null,
            item.nostrZapReceipt || null,
            item.senderComment || null,
          )
        }
      })
      transaction()
      return Promise.resolve()
    } catch (error) {
      return Promise.reject(new StorageError(`Failed to add lnurl metadata: ${errMessage(error)}`, error))
    }
  }

  // ===== Private Helper Methods =====

  private _rowToPayment(row: Row): any {
    let details: any = null
    if (row.lightning_invoice) {
      details = {
        type: "lightning",
        invoice: row.lightning_invoice,
        destinationPubkey: row.lightning_destination_pubkey,
        description: row.lightning_description,
        htlcDetails: row.lightning_htlc_status
          ? {
              paymentHash: row.lightning_payment_hash,
              preimage: row.lightning_preimage || null,
              expiryTime: row.lightning_htlc_expiry_time ?? 0,
              status: row.lightning_htlc_status,
            }
          : (() => {
              throw new StorageError(`htlc_status is required for Lightning payment ${row.id}`)
            })(),
      }

      if (row.lnurl_pay_info) {
        try {
          details.lnurlPayInfo = JSON.parse(row.lnurl_pay_info as string)
        } catch (e) {
          throw new StorageError(`Failed to parse lnurl_pay_info JSON for payment ${row.id}: ${errMessage(e)}`, e)
        }
      }

      if (row.lnurl_withdraw_info) {
        try {
          details.lnurlWithdrawInfo = JSON.parse(row.lnurl_withdraw_info as string)
        } catch (e) {
          throw new StorageError(`Failed to parse lnurl_withdraw_info JSON for payment ${row.id}: ${errMessage(e)}`, e)
        }
      }

      if (row.lnurl_payment_hash) {
        details.lnurlReceiveMetadata = {
          nostrZapRequest: row.lnurl_nostr_zap_request || null,
          nostrZapReceipt: row.lnurl_nostr_zap_receipt || null,
          senderComment: row.lnurl_sender_comment || null,
        }
      }
    } else if (row.withdraw_tx_id) {
      details = { type: "withdraw", txId: row.withdraw_tx_id }
    } else if (row.deposit_tx_id) {
      details = { type: "deposit", txId: row.deposit_tx_id }
    } else if (row.spark) {
      details = {
        type: "spark",
        invoiceDetails: row.spark_invoice_details ? JSON.parse(row.spark_invoice_details as string) : null,
        htlcDetails: row.spark_htlc_details ? JSON.parse(row.spark_htlc_details as string) : null,
        conversionInfo: row.conversion_info ? JSON.parse(row.conversion_info as string) : null,
      }
    } else if (row.token_metadata) {
      details = {
        type: "token",
        metadata: JSON.parse(row.token_metadata as string),
        txHash: row.token_tx_hash,
        txType: row.token_tx_type,
        invoiceDetails: row.token_invoice_details ? JSON.parse(row.token_invoice_details as string) : null,
        conversionInfo: row.conversion_info ? JSON.parse(row.conversion_info as string) : null,
      }
    }

    let method: any = null
    if (row.method) {
      try {
        method = JSON.parse(row.method as string)
      } catch (e) {
        throw new StorageError(`Failed to parse payment method JSON for payment ${row.id}: ${errMessage(e)}`, e)
      }
    }

    return {
      id: row.id,
      paymentType: row.payment_type,
      status: row.status,
      amount: BigInt(row.amount as string),
      fees: BigInt(row.fees as string),
      timestamp: row.timestamp,
      method,
      details,
      conversionDetails: row.conversion_status
        ? { status: row.conversion_status, from: null, to: null }
        : null,
    }
  }

  // ===== Sync Operations =====

  syncAddOutgoingChange(record: any): Promise<bigint> {
    try {
      const transaction = this.db.transaction(() => {
        // This revision is a local queue id for pending rows, not a server revision.
        const revisionRow = this.db
          .query(
            `SELECT CAST(COALESCE(MAX(revision), 0) + 1 AS TEXT) AS revision
          FROM sync_outgoing`,
          )
          .get() as { revision: string }
        const revision = BigInt(revisionRow.revision)

        this.db
          .query(
            `INSERT INTO sync_outgoing (
            record_type,
            data_id,
            schema_version,
            commit_time,
            updated_fields_json,
            revision
          ) VALUES (?, ?, ?, ?, ?, CAST(? AS INTEGER))`,
          )
          .run(
            record.id.type,
            record.id.dataId,
            record.schemaVersion,
            Math.floor(Date.now() / 1000),
            JSON.stringify(record.updatedFields),
            revision.toString(),
          )

        return revision
      })

      return Promise.resolve(transaction())
    } catch (error) {
      return Promise.reject(new StorageError(`Failed to add outgoing change: ${errMessage(error)}`, error))
    }
  }

  syncCompleteOutgoingSync(record: any, localRevision: any): Promise<void> {
    try {
      const transaction = this.db.transaction(() => {
        this.db
          .query(
            `DELETE FROM sync_outgoing
          WHERE record_type = ? AND data_id = ? AND revision = CAST(? AS INTEGER)`,
          )
          .run(record.id.type, record.id.dataId, localRevision.toString())

        this.db
          .query(
            `INSERT OR REPLACE INTO sync_state (
            record_type,
            data_id,
            revision,
            schema_version,
            commit_time,
            data
          ) VALUES (?, ?, CAST(? AS INTEGER), ?, ?, ?)`,
          )
          .run(
            record.id.type,
            record.id.dataId,
            record.revision.toString(),
            record.schemaVersion,
            Math.floor(Date.now() / 1000),
            JSON.stringify(record.data),
          )

        this.db
          .query(`UPDATE sync_revision SET revision = MAX(revision, CAST(? AS INTEGER))`)
          .run(record.revision.toString())
      })
      transaction()
      return Promise.resolve()
    } catch (error) {
      return Promise.reject(new StorageError(`Failed to complete outgoing sync: ${errMessage(error)}`, error))
    }
  }

  syncGetPendingOutgoingChanges(limit: number): Promise<any[]> {
    try {
      const rows = this.db
        .query(
          `SELECT
          o.record_type,
          o.data_id,
          o.schema_version,
          o.commit_time,
          o.updated_fields_json,
          CAST(o.revision AS TEXT) as revision,
          e.schema_version as existing_schema_version,
          e.commit_time as existing_commit_time,
          e.data as existing_data,
          CAST(e.revision AS TEXT) as existing_revision
        FROM sync_outgoing o
        LEFT JOIN sync_state e ON
          o.record_type = e.record_type AND
          o.data_id = e.data_id
        ORDER BY o.revision ASC
        LIMIT ?`,
        )
        .all(limit) as Row[]

      const changes = rows.map((row) => {
        const change = {
          id: { type: row.record_type, dataId: row.data_id },
          schemaVersion: row.schema_version,
          updatedFields: JSON.parse(row.updated_fields_json as string),
          localRevision: BigInt(row.revision as string),
        }
        let parent: any = null
        if (row.existing_data) {
          parent = {
            id: { type: row.record_type, dataId: row.data_id },
            revision: BigInt(row.existing_revision as string),
            schemaVersion: row.existing_schema_version,
            data: JSON.parse(row.existing_data as string),
          }
        }
        return { change, parent }
      })
      return Promise.resolve(changes)
    } catch (error) {
      return Promise.reject(new StorageError(`Failed to get pending outgoing changes: ${errMessage(error)}`, error))
    }
  }

  syncGetLastRevision(): Promise<bigint> {
    try {
      const row = this.db.query(`SELECT CAST(revision AS TEXT) as revision FROM sync_revision`).get() as
        | { revision: string }
        | null
      return Promise.resolve(row ? BigInt(row.revision) : BigInt(0))
    } catch (error) {
      return Promise.reject(new StorageError(`Failed to get last revision: ${errMessage(error)}`, error))
    }
  }

  syncInsertIncomingRecords(records: any[]): Promise<void> {
    try {
      if (!records || records.length === 0) {
        return Promise.resolve()
      }
      const transaction = this.db.transaction(() => {
        const stmt = this.db.query(
          `INSERT OR REPLACE INTO sync_incoming (
            record_type,
            data_id,
            schema_version,
            commit_time,
            data,
            revision
          ) VALUES (?, ?, ?, ?, ?, CAST(? AS INTEGER))`,
        )
        for (const record of records) {
          stmt.run(
            record.id.type,
            record.id.dataId,
            record.schemaVersion,
            Math.floor(Date.now() / 1000),
            JSON.stringify(record.data),
            record.revision.toString(),
          )
        }
      })
      transaction()
      return Promise.resolve()
    } catch (error) {
      return Promise.reject(new StorageError(`Failed to insert incoming records: ${errMessage(error)}`, error))
    }
  }

  syncDeleteIncomingRecord(record: any): Promise<void> {
    try {
      this.db
        .query(
          `DELETE FROM sync_incoming
        WHERE record_type = ?
        AND data_id = ?
        AND revision = CAST(? AS INTEGER)`,
        )
        .run(record.id.type, record.id.dataId, record.revision.toString())
      return Promise.resolve()
    } catch (error) {
      return Promise.reject(new StorageError(`Failed to delete incoming record: ${errMessage(error)}`, error))
    }
  }

  syncGetIncomingRecords(limit: number): Promise<any[]> {
    try {
      const transaction = this.db.transaction(() => {
        const rows = this.db
          .query(
            `SELECT  i.record_type
          ,       i.data_id
          ,       i.schema_version
          ,       i.data
          ,       CAST(i.revision AS TEXT) AS revision
          ,       e.schema_version AS existing_schema_version
          ,       e.commit_time AS existing_commit_time
          ,       e.data AS existing_data
          ,       CAST(e.revision AS TEXT) AS existing_revision
           FROM sync_incoming i
           LEFT JOIN sync_state e ON i.record_type = e.record_type AND i.data_id = e.data_id
           ORDER BY i.revision ASC
           LIMIT ?`,
          )
          .all(limit) as Row[]

        const results = rows.map((row) => {
          const newState = {
            id: { type: row.record_type, dataId: row.data_id },
            revision: BigInt(row.revision as string),
            schemaVersion: row.schema_version,
            data: JSON.parse(row.data as string),
          }
          let oldState: any = null
          if (row.existing_data) {
            oldState = {
              id: { type: row.record_type, dataId: row.data_id },
              revision: BigInt(row.existing_revision as string),
              schemaVersion: row.existing_schema_version,
              data: JSON.parse(row.existing_data as string),
            }
          }
          return { newState, oldState }
        })
        return results
      })
      return Promise.resolve(transaction())
    } catch (error) {
      return Promise.reject(new StorageError(`Failed to get incoming records: ${errMessage(error)}`, error))
    }
  }

  syncGetLatestOutgoingChange(): Promise<any> {
    try {
      const row = this.db
        .query(
          `SELECT
          o.record_type,
          o.data_id,
          o.schema_version,
          o.commit_time,
          o.updated_fields_json,
          CAST(o.revision AS TEXT) AS revision,
          e.schema_version as existing_schema_version,
          e.commit_time as existing_commit_time,
          e.data as existing_data,
          CAST(e.revision AS TEXT) AS existing_revision
        FROM sync_outgoing o
        LEFT JOIN sync_state e ON
          o.record_type = e.record_type AND
          o.data_id = e.data_id
        ORDER BY o.revision DESC
        LIMIT 1`,
        )
        .get() as Row | null

      if (!row) {
        return Promise.resolve(null)
      }

      const change = {
        id: { type: row.record_type, dataId: row.data_id },
        schemaVersion: row.schema_version,
        updatedFields: JSON.parse(row.updated_fields_json as string),
        localRevision: BigInt(row.revision as string),
      }
      let parent: any = null
      if (row.existing_data) {
        parent = {
          id: { type: row.record_type, dataId: row.data_id },
          revision: BigInt(row.existing_revision as string),
          schemaVersion: row.existing_schema_version,
          data: JSON.parse(row.existing_data as string),
        }
      }
      return Promise.resolve({ change, parent })
    } catch (error) {
      return Promise.reject(new StorageError(`Failed to get latest outgoing change: ${errMessage(error)}`, error))
    }
  }

  syncUpdateRecordFromIncoming(record: any): Promise<void> {
    try {
      const transaction = this.db.transaction(() => {
        this.db
          .query(
            `INSERT OR REPLACE INTO sync_state (
            record_type,
            data_id,
            revision,
            schema_version,
            commit_time,
            data
          ) VALUES (?, ?, CAST(? AS INTEGER), ?, ?, ?)`,
          )
          .run(
            record.id.type,
            record.id.dataId,
            record.revision.toString(),
            record.schemaVersion,
            Math.floor(Date.now() / 1000),
            JSON.stringify(record.data),
          )

        this.db
          .query(`UPDATE sync_revision SET revision = MAX(revision, CAST(? AS INTEGER))`)
          .run(record.revision.toString())
      })
      transaction()
      return Promise.resolve()
    } catch (error) {
      return Promise.reject(new StorageError(`Failed to update record from incoming: ${errMessage(error)}`, error))
    }
  }

  // ===== Contact Operations =====

  listContacts(request: any): Promise<any[]> {
    try {
      const offset = request?.offset !== null && request?.offset !== undefined ? request.offset : 0
      const limit = request?.limit !== null && request?.limit !== undefined ? request.limit : 4294967295
      const rows = this.db
        .query(
          `SELECT id, name, payment_identifier AS paymentIdentifier, created_at AS createdAt, updated_at AS updatedAt
        FROM contacts
        ORDER BY name ASC
        LIMIT ? OFFSET ?`,
        )
        .all(limit, offset) as Row[]
      return Promise.resolve(rows)
    } catch (error) {
      return Promise.reject(new StorageError(`Failed to list contacts: ${errMessage(error)}`, error))
    }
  }

  getContact(id: string): Promise<any> {
    try {
      const row = this.db
        .query(
          `SELECT id, name, payment_identifier AS paymentIdentifier, created_at AS createdAt, updated_at AS updatedAt
        FROM contacts
        WHERE id = ?`,
        )
        .get(id) as Row | null
      return Promise.resolve(row || null)
    } catch (error) {
      return Promise.reject(new StorageError(`Failed to get contact: ${errMessage(error)}`, error))
    }
  }

  insertContact(contact: any): Promise<void> {
    try {
      this.db
        .query(
          `INSERT INTO contacts (id, name, payment_identifier, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
          name = excluded.name,
          payment_identifier = excluded.payment_identifier,
          updated_at = excluded.updated_at`,
        )
        .run(contact.id, contact.name, contact.paymentIdentifier, contact.createdAt, contact.updatedAt)
      return Promise.resolve()
    } catch (error) {
      return Promise.reject(new StorageError(`Failed to insert contact: ${errMessage(error)}`, error))
    }
  }

  deleteContact(id: string): Promise<void> {
    try {
      this.db.query("DELETE FROM contacts WHERE id = ?").run(id)
      return Promise.resolve()
    } catch (error) {
      return Promise.reject(new StorageError(`Failed to delete contact: ${errMessage(error)}`, error))
    }
  }
}

/** Extract an error message without leaking secret-shaped material. */
function errMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}
