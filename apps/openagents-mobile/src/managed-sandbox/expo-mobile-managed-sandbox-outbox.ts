import { openDatabaseSync, type SQLiteDatabase } from "expo-sqlite";

import {
  decodeManagedSandboxSupervisionCommand,
  decodeManagedSandboxSupervisionOutcome,
} from "@openagentsinc/managed-sandbox-contract";

import type {
  MobileManagedSandboxOutbox,
  MobileManagedSandboxOutboxRecord,
} from "./mobile-managed-sandbox";

type Row = Readonly<{
  command_ref: string;
  body_json: string;
  outcome_json: string | null;
}>;

const migrate = (database: SQLiteDatabase): void => {
  database.execSync(`
    CREATE TABLE IF NOT EXISTS mobile_managed_sandbox_outbox (
      command_ref TEXT PRIMARY KEY,
      body_json TEXT NOT NULL,
      outcome_json TEXT,
      created_at TEXT NOT NULL
    ) STRICT;
  `);
};

const decodeRecord = (row: Row): MobileManagedSandboxOutboxRecord => {
  const parsed = JSON.parse(row.body_json) as { command?: unknown };
  return {
    command: decodeManagedSandboxSupervisionCommand(parsed.command),
    bodyJson: row.body_json,
    outcome:
      row.outcome_json === null
        ? null
        : decodeManagedSandboxSupervisionOutcome(JSON.parse(row.outcome_json)),
  };
};

export const openExpoMobileManagedSandboxOutbox = (
  databaseName: string,
  now: () => Date = () => new Date(),
): MobileManagedSandboxOutbox => {
  const database = openDatabaseSync(databaseName);
  migrate(database);
  return {
    put: async (record) => {
      const existing = database.getFirstSync<Row>(
        "SELECT command_ref, body_json, outcome_json FROM mobile_managed_sandbox_outbox WHERE command_ref = ?",
        record.command.commandRef,
      );
      if (existing !== null && existing.body_json !== record.bodyJson) {
        throw new Error("managed-sandbox command ref is bound to different bytes");
      }
      if (existing === null) {
        database.runSync(
          "INSERT INTO mobile_managed_sandbox_outbox (command_ref, body_json, outcome_json, created_at) VALUES (?, ?, NULL, ?)",
          record.command.commandRef,
          record.bodyJson,
          now().toISOString(),
        );
      }
    },
    pending: async () =>
      database
        .getAllSync<Row>(
          "SELECT command_ref, body_json, outcome_json FROM mobile_managed_sandbox_outbox WHERE outcome_json IS NULL ORDER BY created_at ASC, command_ref ASC",
        )
        .map(decodeRecord),
    settle: async (commandRef, outcome) => {
      database.runSync(
        "UPDATE mobile_managed_sandbox_outbox SET outcome_json = ? WHERE command_ref = ? AND outcome_json IS NULL",
        JSON.stringify(outcome),
        commandRef,
      );
    },
    close: () => database.closeSync(),
  };
};
