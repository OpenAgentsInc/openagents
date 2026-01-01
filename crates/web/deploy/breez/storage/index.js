/**
 * ES6 module for Web IndexedDB Storage Implementation
 * This provides an ES6 interface to IndexedDB storage for web browsers
 */

class MigrationManager {
  constructor(db, StorageError, logger = null) {
    this.db = db;
    this.StorageError = StorageError;
    this.logger = logger;
    this.migrations = this._getMigrations();
  }

  /**
   * Handle IndexedDB upgrade event - called during database opening
   */
  handleUpgrade(event, oldVersion, newVersion) {
    const db = event.target.result;
    const transaction = event.target.transaction;

    this._log(
      "info",
      `Upgrading IndexedDB from version ${oldVersion} to ${newVersion}`
    );

    try {
      for (let i = oldVersion; i < newVersion; i++) {
        const migration = this.migrations[i];
        if (migration) {
          this._log("debug", `Running migration ${i + 1}: ${migration.name}`);
          migration.upgrade(db, transaction);
        }
      }
      this._log("info", `Database migration completed successfully`);
    } catch (error) {
      this._log(
        "error",
        `Migration failed at version ${oldVersion}: ${error.message}`
      );
      throw new this.StorageError(
        `Migration failed at version ${oldVersion}: ${error.message}`,
        error
      );
    }
  }

  _log(level, message) {
    if (this.logger && typeof this.logger.log === "function") {
      this.logger.log({
        line: message,
        level: level,
      });
    } else if (level === "error") {
      console.error(`[MigrationManager] ${message}`);
    }
  }

  /**
   * Define all database migrations for IndexedDB
   *
   * Each migration is an object with:
   * - name: Description of the migration
   * - upgrade: Function that takes (db, transaction) and creates/modifies object stores
   */
  _getMigrations() {
    return [
      {
        name: "Create initial object stores",
        upgrade: (db) => {
          // Settings store (key-value cache)
          if (!db.objectStoreNames.contains("settings")) {
            db.createObjectStore("settings", { keyPath: "key" });
          }

          // Payments store
          if (!db.objectStoreNames.contains("payments")) {
            const paymentStore = db.createObjectStore("payments", {
              keyPath: "id",
            });
            paymentStore.createIndex("timestamp", "timestamp", {
              unique: false,
            });
            paymentStore.createIndex("paymentType", "paymentType", {
              unique: false,
            });
            paymentStore.createIndex("status", "status", { unique: false });
          }

          // Payment metadata store
          if (!db.objectStoreNames.contains("payment_metadata")) {
            db.createObjectStore("payment_metadata", { keyPath: "paymentId" });
          }

          // Unclaimed deposits store
          if (!db.objectStoreNames.contains("unclaimed_deposits")) {
            const depositStore = db.createObjectStore("unclaimed_deposits", {
              keyPath: ["txid", "vout"],
            });
            depositStore.createIndex("txid", "txid", { unique: false });
          }
        },
      },
      {
        name: "Create invoice index",
        upgrade: (db, transaction) => {
          const paymentStore = transaction.objectStore("payments");
          if (!paymentStore.indexNames.contains("invoice")) {
            paymentStore.createIndex("invoice", "details.invoice", {
              unique: false,
            });
          }
        },
      },
      {
        name: "Convert amount and fees from Number to BigInt for u128 support",
        upgrade: (db, transaction) => {
          const store = transaction.objectStore("payments");
          const getAllRequest = store.getAll();

          getAllRequest.onsuccess = () => {
            const payments = getAllRequest.result;
            let updated = 0;

            payments.forEach((payment) => {
              // Convert amount and fees from Number to BigInt if they're numbers
              let needsUpdate = false;

              if (typeof payment.amount === "number") {
                payment.amount = BigInt(Math.round(payment.amount));
                needsUpdate = true;
              }

              if (typeof payment.fees === "number") {
                payment.fees = BigInt(Math.round(payment.fees));
                needsUpdate = true;
              }

              if (needsUpdate) {
                store.put(payment);
                updated++;
              }
            });

            console.log(`Migrated ${updated} payment records to BigInt format`);
          };
        },
      },
      {
        name: "Add sync tables",
        upgrade: (db, transaction) => {
          if (!db.objectStoreNames.contains("sync_revision")) {
            const syncRevisionStore = db.createObjectStore("sync_revision", {
              keyPath: "id",
            });
            transaction
              .objectStore("sync_revision")
              .add({ id: 1, revision: "0" });
          }

          if (!db.objectStoreNames.contains("sync_outgoing")) {
            db.createObjectStore("sync_outgoing", {
              keyPath: ["type", "dataId", "revision"],
            });
            transaction
              .objectStore("sync_outgoing")
              .createIndex("revision", "revision");
          }

          if (!db.objectStoreNames.contains("sync_incoming")) {
            db.createObjectStore("sync_incoming", {
              keyPath: ["type", "dataId", "revision"],
            });
            transaction
              .objectStore("sync_incoming")
              .createIndex("revision", "revision");
          }

          if (!db.objectStoreNames.contains("sync_state")) {
            db.createObjectStore("sync_state", { keyPath: ["type", "dataId"] });
          }
        }
      },
      {
        name: "Create lnurl_receive_metadata store",
        upgrade: (db) => {
          if (!db.objectStoreNames.contains("lnurl_receive_metadata")) {
            db.createObjectStore("lnurl_receive_metadata", { keyPath: "paymentHash" });
          }
        }
      },
      {
        // Delete all unclaimed deposits to clear old claim_error JSON format.
        // Deposits will be recovered on next sync.
        name: "Clear unclaimed deposits for claim_error format change",
        upgrade: (db, transaction) => {
          if (db.objectStoreNames.contains("unclaimed_deposits")) {
            const store = transaction.objectStore("unclaimed_deposits");
            store.clear();
          }
        }
      }
    ];
  }
}

class StorageError extends Error {
  constructor(message, cause = null) {
    super(message);
    this.name = "StorageError";
    this.cause = cause;

    // Maintain proper stack trace for where our error was thrown (only available on V8)
    if (Error.captureStackTrace) {
      Error.captureStackTrace(this, StorageError);
    }
  }
}

class IndexedDBStorage {
  constructor(dbName = "BreezSDK", logger = null) {
    this.dbName = dbName;
    this.db = null;
    this.migrationManager = null;
    this.logger = logger;
    this.dbVersion = 6; // Current schema version
  }

  /**
   * Initialize the storage - must be called before using other methods
   */
  async initialize() {
    if (this.db) {
      return this;
    }

    if (typeof window === "undefined" || !window.indexedDB) {
      throw new StorageError("IndexedDB is not available in this environment");
    }

    return new Promise((resolve, reject) => {
      const request = indexedDB.open(this.dbName, this.dbVersion);

      request.onerror = () => {
        const error = new StorageError(
          `Failed to open IndexedDB: ${
            request.error?.message || "Unknown error"
          }`,
          request.error
        );
        reject(error);
      };

      request.onsuccess = () => {
        this.db = request.result;
        this.migrationManager = new MigrationManager(
          this.db,
          StorageError,
          this.logger
        );

        // Handle unexpected version changes
        this.db.onversionchange = () => {
          this.db.close();
          this.db = null;
        };

        resolve(this);
      };

      request.onupgradeneeded = (event) => {
        this.db = event.target.result;
        this.migrationManager = new MigrationManager(
          this.db,
          StorageError,
          this.logger
        );

        try {
          this.migrationManager.handleUpgrade(
            event,
            event.oldVersion,
            event.newVersion
          );
        } catch (error) {
          reject(error);
        }
      };
    });
  }

  /**
   * Close the database connection
   */
  close() {
    if (this.db) {
      this.db.close();
      this.db = null;
    }
  }

  // ===== Cache Operations =====

  async getCachedItem(key) {
    if (!this.db) {
      throw new StorageError("Database not initialized");
    }

    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction("settings", "readonly");
      const store = transaction.objectStore("settings");
      const request = store.get(key);

      request.onsuccess = () => {
        const result = request.result;
        resolve(result ? result.value : null);
      };

      request.onerror = () => {
        reject(
          new StorageError(
            `Failed to get cached item '${key}': ${
              request.error?.message || "Unknown error"
            }`,
            request.error
          )
        );
      };
    });
  }

  async setCachedItem(key, value) {
    if (!this.db) {
      throw new StorageError("Database not initialized");
    }

    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction("settings", "readwrite");
      const store = transaction.objectStore("settings");
      const request = store.put({ key, value });

      request.onsuccess = () => resolve();

      request.onerror = () => {
        reject(
          new StorageError(
            `Failed to set cached item '${key}': ${
              request.error?.message || "Unknown error"
            }`,
            request.error
          )
        );
      };
    });
  }

  async deleteCachedItem(key) {
    if (!this.db) {
      throw new StorageError("Database not initialized");
    }

    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction("settings", "readwrite");
      const store = transaction.objectStore("settings");
      const request = store.delete(key);

      request.onsuccess = () => resolve();

      request.onerror = () => {
        reject(
          new StorageError(
            `Failed to delete cached item '${key}': ${
              request.error?.message || "Unknown error"
            }`,
            request.error
          )
        );
      };
    });
  }

  // ===== Payment Operations =====

  async listPayments(request) {
    if (!this.db) {
      throw new StorageError("Database not initialized");
    }

    // Handle null values by using default values
    const actualOffset = request.offset !== null ? request.offset : 0;
    const actualLimit = request.limit !== null ? request.limit : 4294967295; // u32::MAX

    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction(
        ["payments", "payment_metadata", "lnurl_receive_metadata"],
        "readonly"
      );
      const paymentStore = transaction.objectStore("payments");
      const metadataStore = transaction.objectStore("payment_metadata");
      const lnurlReceiveMetadataStore = transaction.objectStore("lnurl_receive_metadata");

      const payments = [];
      let count = 0;
      let skipped = 0;

      // Determine sort order - "prev" for descending (default), "next" for ascending
      const cursorDirection = request.sortAscending ? "next" : "prev";

      // Use cursor to iterate through payments ordered by timestamp
      const cursorRequest = paymentStore
        .index("timestamp")
        .openCursor(null, cursorDirection);

      cursorRequest.onsuccess = (event) => {
        const cursor = event.target.result;

        if (!cursor || count >= actualLimit) {
          resolve(payments);
          return;
        }

        const payment = cursor.value;

        // Apply filters
        if (!this._matchesFilters(payment, request)) {
          cursor.continue();
          return;
        }

        if (skipped < actualOffset) {
          skipped++;
          cursor.continue();
          return;
        }

        // Get metadata for this payment
        const metadataRequest = metadataStore.get(payment.id);
        metadataRequest.onsuccess = () => {
          const metadata = metadataRequest.result;
          const paymentWithMetadata = this._mergePaymentMetadata(
            payment,
            metadata
          );
          
          // Fetch lnurl receive metadata if it's a lightning payment
          this._fetchLnurlReceiveMetadata(paymentWithMetadata, lnurlReceiveMetadataStore)
            .then((mergedPayment) => {
              payments.push(mergedPayment);
              count++;
              cursor.continue();
            })
            .catch(() => {
              // Continue without lnurl receive metadata if fetch fails
              payments.push(paymentWithMetadata);
              count++;
              cursor.continue();
            });
        };
        metadataRequest.onerror = () => {
          // Continue without metadata if it fails
          payments.push(payment);
          count++;
          cursor.continue();
        };
      };

      cursorRequest.onerror = () => {
        reject(
          new StorageError(
            `Failed to list payments (request: ${JSON.stringify(request)}: ${
              cursorRequest.error?.message || "Unknown error"
            }`,
            cursorRequest.error
          )
        );
      };
    });
  }

  async insertPayment(payment) {
    if (!this.db) {
      throw new StorageError("Database not initialized");
    }

    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction("payments", "readwrite");
      const store = transaction.objectStore("payments");

      // Ensure details and method are serialized properly
      const paymentToStore = {
        ...payment,
        details: payment.details ? JSON.stringify(payment.details) : null,
        method: payment.method ? JSON.stringify(payment.method) : null,
      };

      const request = store.put(paymentToStore);
      request.onsuccess = () => resolve();
      request.onerror = () => {
        reject(
          new StorageError(
            `Failed to insert payment '${payment.id}': ${
              request.error?.message || "Unknown error"
            }`,
            request.error
          )
        );
      };
    });
  }

  async getPaymentById(id) {
    if (!this.db) {
      throw new StorageError("Database not initialized");
    }

    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction(
        ["payments", "payment_metadata", "lnurl_receive_metadata"],
        "readonly"
      );
      const paymentStore = transaction.objectStore("payments");
      const metadataStore = transaction.objectStore("payment_metadata");
      const lnurlReceiveMetadataStore = transaction.objectStore("lnurl_receive_metadata");

      const paymentRequest = paymentStore.get(id);

      paymentRequest.onsuccess = () => {
        const payment = paymentRequest.result;
        if (!payment) {
          reject(new StorageError(`Payment with id '${id}' not found`));
          return;
        }

        // Get metadata for this payment
        const metadataRequest = metadataStore.get(id);
        metadataRequest.onsuccess = () => {
          const metadata = metadataRequest.result;
          const paymentWithMetadata = this._mergePaymentMetadata(
            payment,
            metadata
          );
          
          // Fetch lnurl receive metadata if it's a lightning payment
          this._fetchLnurlReceiveMetadata(paymentWithMetadata, lnurlReceiveMetadataStore)
            .then(resolve)
            .catch(() => {
              // Continue without lnurl receive metadata if fetch fails
              resolve(paymentWithMetadata);
            });
        };
        metadataRequest.onerror = () => {
          // Return payment without metadata if metadata fetch fails
          resolve(payment);
        };
      };

      paymentRequest.onerror = () => {
        reject(
          new StorageError(
            `Failed to get payment by id '${id}': ${
              paymentRequest.error?.message || "Unknown error"
            }`,
            paymentRequest.error
          )
        );
      };
    });
  }

  async getPaymentByInvoice(invoice) {
    if (!this.db) {
      throw new StorageError("Database not initialized");
    }

    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction(
        ["payments", "payment_metadata", "lnurl_receive_metadata"],
        "readonly"
      );
      const paymentStore = transaction.objectStore("payments");
      const invoiceIndex = paymentStore.index("invoice");
      const metadataStore = transaction.objectStore("payment_metadata");
      const lnurlReceiveMetadataStore = transaction.objectStore("lnurl_receive_metadata");

      const paymentRequest = invoiceIndex.get(invoice);

      paymentRequest.onsuccess = () => {
        const payment = paymentRequest.result;
        if (!payment) {
          resolve(null);
          return;
        }

        // Get metadata for this payment
        const metadataRequest = metadataStore.get(payment.id);
        metadataRequest.onsuccess = () => {
          const metadata = metadataRequest.result;
          const paymentWithMetadata = this._mergePaymentMetadata(
            payment,
            metadata
          );
          
          // Fetch lnurl receive metadata if it's a lightning payment
          this._fetchLnurlReceiveMetadata(paymentWithMetadata, lnurlReceiveMetadataStore)
            .then(resolve)
            .catch(() => {
              // Continue without lnurl receive metadata if fetch fails
              resolve(paymentWithMetadata);
            });
        };
        metadataRequest.onerror = () => {
          // Return payment without metadata if metadata fetch fails
          resolve(payment);
        };
      };

      paymentRequest.onerror = () => {
        reject(
          new StorageError(
            `Failed to get payment by invoice '${invoice}': ${
              paymentRequest.error?.message || "Unknown error"
            }`,
            paymentRequest.error
          )
        );
      };
    });
  }

  async setPaymentMetadata(paymentId, metadata) {
    if (!this.db) {
      throw new StorageError("Database not initialized");
    }

    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction("payment_metadata", "readwrite");
      const store = transaction.objectStore("payment_metadata");

      const metadataToStore = {
        paymentId,
        lnurlPayInfo: metadata.lnurlPayInfo
          ? JSON.stringify(metadata.lnurlPayInfo)
          : null,
        lnurlWithdrawInfo: metadata.lnurlWithdrawInfo
          ? JSON.stringify(metadata.lnurlWithdrawInfo)
          : null,
        lnurlDescription: metadata.lnurlDescription,
      };

      const request = store.put(metadataToStore);
      request.onsuccess = () => resolve();
      request.onerror = () => {
        reject(
          new StorageError(
            `Failed to set payment metadata for '${paymentId}': ${
              request.error?.message || "Unknown error"
            }`,
            request.error
          )
        );
      };
    });
  }

  // ===== Deposit Operations =====

  async addDeposit(txid, vout, amountSats) {
    if (!this.db) {
      throw new StorageError("Database not initialized");
    }

    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction(
        "unclaimed_deposits",
        "readwrite"
      );
      const store = transaction.objectStore("unclaimed_deposits");

      const depositToStore = {
        txid,
        vout,
        amountSats,
        claimError: null,
        refundTx: null,
        refundTxId: null,
      };

      const request = store.put(depositToStore);
      request.onsuccess = () => resolve();
      request.onerror = () => {
        reject(
          new StorageError(
            `Failed to add deposit '${txid}:${vout}': ${
              request.error?.message || "Unknown error"
            }`,
            request.error
          )
        );
      };
    });
  }

  async deleteDeposit(txid, vout) {
    if (!this.db) {
      throw new StorageError("Database not initialized");
    }

    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction(
        "unclaimed_deposits",
        "readwrite"
      );
      const store = transaction.objectStore("unclaimed_deposits");
      const request = store.delete([txid, vout]);

      request.onsuccess = () => resolve();
      request.onerror = () => {
        reject(
          new StorageError(
            `Failed to delete deposit '${txid}:${vout}': ${
              request.error?.message || "Unknown error"
            }`,
            request.error
          )
        );
      };
    });
  }

  async listDeposits() {
    if (!this.db) {
      throw new StorageError("Database not initialized");
    }

    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction("unclaimed_deposits", "readonly");
      const store = transaction.objectStore("unclaimed_deposits");
      const request = store.getAll();

      request.onsuccess = () => {
        const deposits = request.result.map((row) => ({
          txid: row.txid,
          vout: row.vout,
          amountSats: row.amountSats,
          claimError: row.claimError ? JSON.parse(row.claimError) : null,
          refundTx: row.refundTx,
          refundTxId: row.refundTxId,
        }));
        resolve(deposits);
      };

      request.onerror = () => {
        reject(
          new StorageError(
            `Failed to list deposits: ${
              request.error?.message || "Unknown error"
            }`,
            request.error
          )
        );
      };
    });
  }

  async updateDeposit(txid, vout, payload) {
    if (!this.db) {
      throw new StorageError("Database not initialized");
    }

    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction(
        "unclaimed_deposits",
        "readwrite"
      );
      const store = transaction.objectStore("unclaimed_deposits");

      // First get the existing deposit
      const getRequest = store.get([txid, vout]);

      getRequest.onsuccess = () => {
        const existingDeposit = getRequest.result;
        if (!existingDeposit) {
          // Deposit doesn't exist, just resolve (matches SQLite behavior)
          resolve();
          return;
        }

        let updatedDeposit = { ...existingDeposit };

        if (payload.type === "claimError") {
          updatedDeposit.claimError = JSON.stringify(payload.error);
          updatedDeposit.refundTx = null;
          updatedDeposit.refundTxId = null;
        } else if (payload.type === "refund") {
          updatedDeposit.refundTx = payload.refundTx;
          updatedDeposit.refundTxId = payload.refundTxid;
          updatedDeposit.claimError = null;
        } else {
          reject(new StorageError(`Unknown payload type: ${payload.type}`));
          return;
        }

        const putRequest = store.put(updatedDeposit);
        putRequest.onsuccess = () => resolve();
        putRequest.onerror = () => {
          reject(
            new StorageError(
              `Failed to update deposit '${txid}:${vout}': ${
                putRequest.error?.message || "Unknown error"
              }`,
              putRequest.error
            )
          );
        };
      };

      getRequest.onerror = () => {
        reject(
          new StorageError(
            `Failed to get deposit '${txid}:${vout}' for update: ${
              getRequest.error?.message || "Unknown error"
            }`,
            getRequest.error
          )
        );
      };
    });
  }

  async setLnurlMetadata(metadata) {
    if (!this.db) {
      throw new StorageError("Database not initialized");
    }

    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction(
        "lnurl_receive_metadata",
        "readwrite"
      );
      const store = transaction.objectStore("lnurl_receive_metadata");

      let completed = 0;
      const total = metadata.length;

      if (total === 0) {
        resolve();
        return;
      }

      for (const item of metadata) {
        const request = store.put({
          paymentHash: item.paymentHash,
          nostrZapRequest: item.nostrZapRequest || null,
          nostrZapReceipt: item.nostrZapReceipt || null,
          senderComment: item.senderComment || null,
        });

        request.onsuccess = () => {
          completed++;
          if (completed === total) {
            resolve();
          }
        };

        request.onerror = () => {
          reject(
            new StorageError(
              `Failed to add lnurl metadata for payment hash '${item.paymentHash}': ${
                request.error?.message || "Unknown error"
              }`,
              request.error
            )
          );
        };
      }
    });
  }

  async syncAddOutgoingChange(record) {
    if (!this.db) {
      throw new StorageError("Database not initialized");
    }

    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction(
        ["sync_outgoing", "sync_revision"],
        "readwrite"
      );

      // Get the next revision
      const revisionStore = transaction.objectStore("sync_revision");
      const getRevisionRequest = revisionStore.get(1);

      getRevisionRequest.onsuccess = () => {
        const revisionData = getRevisionRequest.result || {
          id: 1,
          revision: "0",
        };
        const nextRevision = BigInt(revisionData.revision) + BigInt(1);

        // Update the revision
        const updateRequest = revisionStore.put({
          id: 1,
          revision: nextRevision.toString(),
        });

        updateRequest.onsuccess = () => {
          const outgoingStore = transaction.objectStore("sync_outgoing");

          const storeRecord = {
            type: record.id.type,
            dataId: record.id.dataId,
            revision: Number(nextRevision),
            record: {
              ...record,
              revision: nextRevision,
            },
          };

          const addRequest = outgoingStore.add(storeRecord);

          addRequest.onsuccess = () => {
            // Wait for transaction to complete before resolving
            transaction.oncomplete = () => {
              resolve(nextRevision);
            };
          };

          addRequest.onerror = (event) => {
            reject(
              new StorageError(
                `Failed to add outgoing change: ${event.target.error.message}`
              )
            );
          };
        };

        updateRequest.onerror = (event) => {
          reject(
            new StorageError(
              `Failed to update revision: ${event.target.error.message}`
            )
          );
        };
      };

      getRevisionRequest.onerror = (event) => {
        reject(
          new StorageError(
            `Failed to get revision: ${event.target.error.message}`
          )
        );
      };

      transaction.onerror = (event) => {
        reject(
          new StorageError(`Transaction failed: ${event.target.error.message}`)
        );
      };
    });
  }

  async syncCompleteOutgoingSync(record) {
    if (!this.db) {
      throw new StorageError("Database not initialized");
    }

    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction(
        ["sync_outgoing", "sync_state"],
        "readwrite"
      );
      const outgoingStore = transaction.objectStore("sync_outgoing");
      const stateStore = transaction.objectStore("sync_state");

      const deleteRequest = outgoingStore.delete([
        record.id.type,
        record.id.dataId,
        Number(record.revision),
      ]);

      deleteRequest.onsuccess = () => {
        const stateRecord = {
          type: record.id.type,
          dataId: record.id.dataId,
          record: record,
        };
        stateStore.put(stateRecord);
        resolve();
      };

      deleteRequest.onerror = (event) => {
        reject(
          new StorageError(
            `Failed to complete outgoing sync: ${event.target.error.message}`
          )
        );
      };
    });
  }

  async syncGetPendingOutgoingChanges(limit) {
    if (!this.db) {
      throw new StorageError("Database not initialized");
    }

    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction(
        ["sync_outgoing", "sync_state"],
        "readonly"
      );
      const outgoingStore = transaction.objectStore("sync_outgoing");
      const stateStore = transaction.objectStore("sync_state");

      // Get pending outgoing changes (all records in this store are pending)
      // Use revision index to order by revision ascending
      const revisionIndex = outgoingStore.index("revision");
      const request = revisionIndex.openCursor(null, "next");
      const changes = [];
      let count = 0;

      request.onsuccess = (event) => {
        const cursor = event.target.result;
        if (cursor && count < limit) {
          const storeRecord = cursor.value;
          const change = storeRecord.record;

          // Look up parent record if it exists
          const stateRequest = stateStore.get([
            storeRecord.type,
            storeRecord.dataId,
          ]);
          stateRequest.onsuccess = () => {
            const stateRecord = stateRequest.result;
            const parent = stateRecord ? stateRecord.record : null;

            changes.push({
              change: change,
              parent: parent,
            });

            count++;
            cursor.continue();
          };

          stateRequest.onerror = () => {
            changes.push({
              change: change,
              parent: null,
            });

            count++;
            cursor.continue();
          };
        } else {
          resolve(changes);
        }
      };

      request.onerror = (event) => {
        reject(
          new StorageError(
            `Failed to get pending outgoing changes: ${event.target.error.message}`
          )
        );
      };
    });
  }

  async syncGetLastRevision() {
    if (!this.db) {
      throw new StorageError("Database not initialized");
    }

    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction("sync_revision", "readonly");
      const store = transaction.objectStore("sync_revision");
      const request = store.get(1);

      request.onsuccess = () => {
        const result = request.result || { id: 1, revision: "0" };
        resolve(BigInt(result.revision));
      };

      request.onerror = (event) => {
        reject(
          new StorageError(
            `Failed to get last revision: ${event.target.error.message}`
          )
        );
      };
    });
  }

  async syncInsertIncomingRecords(records) {
    if (!this.db) {
      throw new StorageError("Database not initialized");
    }

    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction(["sync_incoming"], "readwrite");
      const store = transaction.objectStore("sync_incoming");

      // Add each record to the incoming store
      let recordsProcessed = 0;

      for (const record of records) {
        const storeRecord = {
          type: record.id.type,
          dataId: record.id.dataId,
          revision: Number(record.revision),
          record: record,
        };

        const request = store.put(storeRecord);

        request.onsuccess = () => {
          recordsProcessed++;
          if (recordsProcessed === records.length) {
            resolve();
          }
        };

        request.onerror = (event) => {
          reject(
            new StorageError(
              `Failed to insert incoming record: ${event.target.error.message}`
            )
          );
        };
      }

      // If no records were provided
      if (records.length === 0) {
        resolve();
      }
    });
  }

  async syncDeleteIncomingRecord(record) {
    if (!this.db) {
      throw new StorageError("Database not initialized");
    }

    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction(["sync_incoming"], "readwrite");
      const store = transaction.objectStore("sync_incoming");

      const key = [record.id.type, record.id.dataId, Number(record.revision)];
      const request = store.delete(key);

      request.onsuccess = () => {
        resolve();
      };

      request.onerror = (event) => {
        reject(
          new StorageError(
            `Failed to delete incoming record: ${event.target.error.message}`
          )
        );
      };
    });
  }

  async syncRebasePendingOutgoingRecords(revision) {
    if (!this.db) {
      throw new StorageError("Database not initialized");
    }

    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction(
        ["sync_outgoing", "sync_revision"],
        "readwrite"
      );
      const outgoingStore = transaction.objectStore("sync_outgoing");
      const revisionStore = transaction.objectStore("sync_revision");

      // Get the last revision from sync_revision table
      const getRevisionRequest = revisionStore.get(1);

      getRevisionRequest.onsuccess = () => {
        const revisionData = getRevisionRequest.result || {
          id: 1,
          revision: "0",
        };
        const lastRevision = BigInt(revisionData.revision);

        // Calculate the difference
        const diff = revision - lastRevision;

        if (diff <= BigInt(0)) {
          // No rebase needed
          resolve();
          return;
        }

        // Get all records from sync_outgoing and update their revisions
        const getAllRequest = outgoingStore.getAll();

        getAllRequest.onsuccess = () => {
          const records = getAllRequest.result;
          let updatesCompleted = 0;

          if (records.length === 0) {
            resolve();
            return;
          }

          for (const storeRecord of records) {
            // Delete the old record
            const oldKey = [
              storeRecord.type,
              storeRecord.dataId,
              storeRecord.revision,
            ];
            outgoingStore.delete(oldKey);

            // Update revision in both the key and the nested record
            const newRevision = storeRecord.record.revision + diff;
            const updatedRecord = {
              type: storeRecord.type,
              dataId: storeRecord.dataId,
              revision: Number(newRevision),
              record: {
                ...storeRecord.record,
                revision: newRevision,
              },
            };

            // Add the updated record
            const putRequest = outgoingStore.put(updatedRecord);

            putRequest.onsuccess = () => {
              updatesCompleted++;
              if (updatesCompleted === records.length) {
                resolve();
              }
            };

            putRequest.onerror = (event) => {
              reject(
                new StorageError(
                  `Failed to rebase outgoing record: ${event.target.error.message}`
                )
              );
            };
          }
        };

        getAllRequest.onerror = (event) => {
          reject(
            new StorageError(
              `Failed to get outgoing records for rebase: ${event.target.error.message}`
            )
          );
        };
      };

      getRevisionRequest.onerror = (event) => {
        reject(
          new StorageError(
            `Failed to get last revision: ${event.target.error.message}`
          )
        );
      };
    });
  }

  async syncGetIncomingRecords(limit) {
    if (!this.db) {
      throw new StorageError("Database not initialized");
    }

    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction(
        ["sync_incoming", "sync_state"],
        "readonly"
      );
      const incomingStore = transaction.objectStore("sync_incoming");
      const stateStore = transaction.objectStore("sync_state");

      // Get records up to the limit, ordered by revision
      const revisionIndex = incomingStore.index("revision");
      const request = revisionIndex.openCursor(null, "next");
      const records = [];
      let count = 0;

      request.onsuccess = (event) => {
        const cursor = event.target.result;
        if (cursor && count < limit) {
          const storeRecord = cursor.value;
          const newState = storeRecord.record;

          // Look for parent record
          const stateRequest = stateStore.get([
            storeRecord.type,
            storeRecord.dataId,
          ]);

          stateRequest.onsuccess = () => {
            const stateRecord = stateRequest.result;
            const oldState = stateRecord ? stateRecord.record : null;

            records.push({
              newState: newState,
              oldState: oldState,
            });

            count++;
            cursor.continue();
          };

          stateRequest.onerror = () => {
            records.push({
              newState: newState,
              oldState: null,
            });

            count++;
            cursor.continue();
          };
        } else {
          resolve(records);
        }
      };

      request.onerror = (event) => {
        reject(
          new StorageError(
            `Failed to get incoming records: ${event.target.error.message}`
          )
        );
      };
    });
  }

  async syncGetLatestOutgoingChange() {
    if (!this.db) {
      throw new StorageError("Database not initialized");
    }

    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction(
        ["sync_outgoing", "sync_state"],
        "readonly"
      );
      const outgoingStore = transaction.objectStore("sync_outgoing");
      const stateStore = transaction.objectStore("sync_state");

      // Get the highest revision record
      const index = outgoingStore.index("revision");
      const request = index.openCursor(null, "prev");

      request.onsuccess = (event) => {
        const cursor = event.target.result;
        if (cursor) {
          const storeRecord = cursor.value;
          const change = storeRecord.record;

          // Get the parent record
          const stateRequest = stateStore.get([
            storeRecord.type,
            storeRecord.dataId,
          ]);

          stateRequest.onsuccess = () => {
            const stateRecord = stateRequest.result;
            const parent = stateRecord ? stateRecord.record : null;

            resolve({
              change: change,
              parent: parent,
            });
          };

          stateRequest.onerror = () => {
            resolve({
              change: change,
              parent: null,
            });
          };
        } else {
          // No records found
          resolve(null);
        }
      };

      request.onerror = (event) => {
        reject(
          new StorageError(
            `Failed to get latest outgoing change: ${event.target.error.message}`
          )
        );
      };
    });
  }

  async syncUpdateRecordFromIncoming(record) {
    if (!this.db) {
      throw new StorageError("Database not initialized");
    }

    return new Promise((resolve, reject) => {
      const transaction = this.db.transaction(["sync_state"], "readwrite");
      const stateStore = transaction.objectStore("sync_state");

      const storeRecord = {
        type: record.id.type,
        dataId: record.id.dataId,
        record: record,
      };

      const request = stateStore.put(storeRecord);

      request.onsuccess = () => {
        resolve();
      };

      request.onerror = (event) => {
        reject(
          new StorageError(
            `Failed to update record from incoming: ${event.target.error.message}`
          )
        );
      };
    });
  }

  // ===== Private Helper Methods =====

  _matchesFilters(payment, request) {
    // Filter by payment type
    if (request.typeFilter && request.typeFilter.length > 0) {
      if (!request.typeFilter.includes(payment.paymentType)) {
        return false;
      }
    }

    // Filter by status
    if (request.statusFilter && request.statusFilter.length > 0) {
      if (!request.statusFilter.includes(payment.status)) {
        return false;
      }
    }

    // Filter by timestamp range
    if (request.fromTimestamp !== null && request.fromTimestamp !== undefined) {
      if (payment.timestamp < request.fromTimestamp) {
        return false;
      }
    }

    if (request.toTimestamp !== null && request.toTimestamp !== undefined) {
      if (payment.timestamp >= request.toTimestamp) {
        return false;
      }
    }

    // Filter by Spark HTLC status
    if (
      request.sparkHtlcStatusFilter &&
      request.sparkHtlcStatusFilter.length > 0
    ) {
      let details = null;

      // Parse details if it's a string (stored in IndexedDB)
      if (payment.details && typeof payment.details === "string") {
        try {
          details = JSON.parse(payment.details);
        } catch (e) {
          // If parsing fails, treat as no details
          details = null;
        }
      } else {
        details = payment.details;
      }

      // Only Spark payments can have HTLC details
      if (!details || details.type !== "spark" || !details.htlcDetails) {
        return false;
      }

      if (!request.sparkHtlcStatusFilter.includes(details.htlcDetails.status)) {
        return false;
      }
    }

    // Filter by payment details/method
    if (request.assetFilter) {
      const assetFilter = request.assetFilter;
      let details = null;

      // Parse details if it's a string (stored in IndexedDB)
      if (payment.details && typeof payment.details === "string") {
        try {
          details = JSON.parse(payment.details);
        } catch (e) {
          // If parsing fails, treat as no details
          details = null;
        }
      } else {
        details = payment.details;
      }

      if (!details) {
        return false;
      }

      if (assetFilter.type === "bitcoin" && details.type === "token") {
        return false;
      }

      if (assetFilter.type === "token") {
        if (details.type !== "token") {
          return false;
        }

        // Check token identifier if specified
        if (assetFilter.tokenIdentifier) {
          if (
            !details.metadata ||
            details.metadata.identifier !== assetFilter.tokenIdentifier
          ) {
            return false;
          }
        }
      }
    }

    return true;
  }

  _mergePaymentMetadata(payment, metadata) {
    let details = null;
    if (payment.details) {
      try {
        details = JSON.parse(payment.details);
      } catch (e) {
        throw new StorageError(
          `Failed to parse payment details JSON for payment ${payment.id}: ${e.message}`,
          e
        );
      }
    }

    let method = null;
    if (payment.method) {
      try {
        method = JSON.parse(payment.method);
      } catch (e) {
        throw new StorageError(
          `Failed to parse payment method JSON for payment ${payment.id}: ${e.message}`,
          e
        );
      }
    }

    // If this is a Lightning payment and we have metadata
    if (metadata && details && details.type == "lightning") {
      if (metadata.lnurlDescription && !details.description) {
        details.description = metadata.lnurlDescription;
      }
      // If lnurlPayInfo exists, parse and add to details
      if (metadata.lnurlPayInfo) {
        try {
          details.lnurlPayInfo = JSON.parse(metadata.lnurlPayInfo);
        } catch (e) {
          throw new StorageError(
            `Failed to parse lnurlPayInfo JSON for payment ${payment.id}: ${e.message}`,
            e
          );
        }
      }
      // If lnurlWithdrawInfo exists, parse and add to details
      if (metadata.lnurlWithdrawInfo) {
        try {
          details.lnurlWithdrawInfo = JSON.parse(metadata.lnurlWithdrawInfo);
        } catch (e) {
          throw new StorageError(
            `Failed to parse lnurlWithdrawInfo JSON for payment ${payment.id}: ${e.message}`,
            e
          );
        }
      }
    }

    return {
      id: payment.id,
      paymentType: payment.paymentType,
      status: payment.status,
      amount: payment.amount,
      fees: payment.fees,
      timestamp: payment.timestamp,
      method,
      details,
    };
  }

  _fetchLnurlReceiveMetadata(payment, lnurlReceiveMetadataStore) {
    // Only fetch for lightning payments with a payment hash
    if (!payment.details || payment.details.type !== "lightning" || !payment.details.paymentHash) {
      return Promise.resolve(payment);
    }

    if (!lnurlReceiveMetadataStore) {
      return Promise.resolve(payment);
    }

    return new Promise((resolve, reject) => {
      const lnurlReceiveRequest = lnurlReceiveMetadataStore.get(payment.details.paymentHash);
      
      lnurlReceiveRequest.onsuccess = () => {
        const lnurlReceiveMetadata = lnurlReceiveRequest.result;
        if (lnurlReceiveMetadata && (lnurlReceiveMetadata.nostrZapRequest || lnurlReceiveMetadata.senderComment)) {
          payment.details.lnurlReceiveMetadata = {
            nostrZapRequest: lnurlReceiveMetadata.nostrZapRequest || null,
            nostrZapReceipt: lnurlReceiveMetadata.nostrZapReceipt || null,
            senderComment: lnurlReceiveMetadata.senderComment || null,
          };
        }
        resolve(payment);
      };
      
      lnurlReceiveRequest.onerror = () => {
        // Continue without lnurlReceiveMetadata if fetch fails
        reject(new Error("Failed to fetch lnurl receive metadata"));
      };
    });
  }
}

export async function createDefaultStorage(
  dbName = "BreezSdkSpark",
  logger = null
) {
  const storage = new IndexedDBStorage(dbName, logger);
  await storage.initialize();
  return storage;
}

export { IndexedDBStorage, StorageError };
