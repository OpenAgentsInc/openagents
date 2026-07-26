import { SQL } from "@openagentsinc/postgres-runtime";
import { afterEach, describe, expect, test } from "vite-plus/test";
import { hasLocalPostgres, startLocalPostgres, type LocalPostgres } from "./local-postgres.js";

describe.skipIf(!hasLocalPostgres())("shared local Postgres", () => {
  const clusters: Array<LocalPostgres> = [];

  afterEach(async () => {
    await Promise.all(clusters.splice(0).map((cluster) => cluster.stop()));
  });

  test("shares one postmaster while isolating each caller database", async () => {
    const [first, second] = await Promise.all([startLocalPostgres(), startLocalPostgres()]);
    clusters.push(first, second);

    expect(first.port).toBe(second.port);
    expect(first.dataDir).toBe(second.dataDir);
    expect(first.user).not.toBe(second.user);
    expect(first.url).not.toBe(second.url);

    const firstSql = SQL({ url: first.url, max: 1 });
    const secondSql = SQL({ url: second.url, max: 1 });
    try {
      await firstSql.unsafe("CREATE TABLE isolated_row (value text NOT NULL)");
      await firstSql.unsafe("INSERT INTO isolated_row (value) VALUES ('first')");
      const [{ value }] = await firstSql`SELECT value FROM isolated_row`;
      const [{ relation }] = await secondSql`
        SELECT to_regclass('isolated_row')::text AS relation
      `;

      expect(value).toBe("first");
      expect(relation).toBeNull();
    } finally {
      await firstSql.end();
      await secondSql.end();
    }

    await Promise.all(clusters.splice(0).map((cluster) => cluster.stop()));
    const stoppedSql = SQL({ url: first.url, max: 1 });
    try {
      await expect(stoppedSql`SELECT 1`).rejects.toThrow();
    } finally {
      await stoppedSql.end();
    }
  });
});
