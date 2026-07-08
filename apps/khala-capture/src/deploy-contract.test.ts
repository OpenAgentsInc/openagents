// Guards the env contract between scripts/deploy-cloudrun.sh and the capture
// daemon: the exact env var names the deploy sets must resolve to a valid
// Cloud SQL Auth Connector socket config (#8554). If someone renames a var on
// either side, this fails instead of shipping a daemon that cannot connect.
import { describe, expect, test } from "bun:test"
import { captureConfigFromEnv } from "@openagentsinc/khala-sync-server/capture"

describe("khala-capture deploy env contract", () => {
  test("the deploy-cloudrun.sh env set resolves to a connector socket config", () => {
    // Mirror scripts/deploy-cloudrun.sh (prod): PGHOST is the connector dir,
    // secrets arrive as PGPASSWORD + KHALA_SYNC_HUB_TOKEN, no admin token.
    const config = captureConfigFromEnv({
      PGHOST: "/cloudsql/openagentsgemini:us-central1:khala-sync-pg",
      PGUSER: "khala_capture",
      PGPASSWORD: "role-password",
      PGDATABASE: "khala_sync_prod",
      KHALA_SYNC_HUB_APPEND_URL:
        "https://khala-live-hub-ezxz4mgdsq-uc.a.run.app/append",
      KHALA_SYNC_HUB_TOKEN: "livehub-shared-bearer",
      PORT: "8080",
    })

    expect(config.databaseUrl).toBeUndefined()
    expect(config.socket).toEqual({
      socketPath:
        "/cloudsql/openagentsgemini:us-central1:khala-sync-pg/.s.PGSQL.5432",
      username: "khala_capture",
      password: "role-password",
      database: "khala_sync_prod",
    })
    expect(config.hubAppendUrl).toBe(
      "https://khala-live-hub-ezxz4mgdsq-uc.a.run.app/append",
    )
    expect(config.hubToken).toBe("livehub-shared-bearer")
  })

  test("a missing PGPASSWORD still resolves (password may be empty for the socket)", () => {
    // The deploy always mounts PGPASSWORD, but socket-mode selection must not
    // hinge on it (username + database + connector PGHOST are what matter).
    const config = captureConfigFromEnv({
      PGHOST: "/cloudsql/openagentsgemini:us-central1:khala-sync-pg",
      PGUSER: "khala_capture",
      PGDATABASE: "khala_sync_staging",
      KHALA_SYNC_HUB_APPEND_URL: "https://hub.example/append",
      KHALA_SYNC_HUB_TOKEN: "t",
    })
    expect(config.socket?.database).toBe("khala_sync_staging")
    expect(config.socket?.password).toBe("")
  })
})
