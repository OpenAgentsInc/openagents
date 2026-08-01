import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { collectAdmissionDisableReceipt } from "./livekit-admission-disable.mjs";
import { validateDeploymentBundle } from "./livekit-ops-policy.mjs";

const bundle = validateDeploymentBundle(
  JSON.parse(readFileSync(new URL("../../infra/livekit/bundle.json", import.meta.url), "utf8")),
);

const service = () => ({
  status: {
    latestReadyRevisionName: "openagents-monolith-00400-test",
    traffic: [{ revisionName: "openagents-monolith-00400-test", percent: 100 }],
    conditions: [{ type: "Ready", status: "True" }],
  },
});

const revision = (admission = "false") => ({
  metadata: { name: "openagents-monolith-00400-test" },
  spec: {
    containers: [{ env: [{ name: "SARAH_LIVEKIT_NEW_ADMISSIONS_ENABLED", value: admission }] }],
  },
  status: { conditions: [{ type: "Ready", status: "True" }] },
});

const withDatabaseEnvironment = (run) => {
  const held = Object.fromEntries(
    ["PGHOST", "PGUSER", "PGPASSWORD", "PGDATABASE"].map((name) => [name, process.env[name]]),
  );
  Object.assign(process.env, {
    PGHOST: "127.0.0.1",
    PGUSER: "migration-reader",
    PGPASSWORD: "not-observed",
    PGDATABASE: "khala_sync_prod",
  });
  try {
    return run();
  } finally {
    for (const [name, value] of Object.entries(held)) {
      if (value === undefined) delete process.env[name];
      else process.env[name] = value;
    }
  }
};

const collect = (cloudRunService = service(), counts = "0,0\n", cloudRunRevision = revision()) =>
  withDatabaseEnvironment(() =>
    collectAdmissionDisableReceipt({
      bundle,
      deployedRevision: "a".repeat(40),
      now: () => "2026-08-01T12:00:00.000Z",
      runCommand(bin, args) {
        if (bin === "gcloud") {
          if (args[1] === "services") {
            assert.deepEqual(args.slice(0, 4), [
              "run",
              "services",
              "describe",
              "openagents-monolith",
            ]);
            return { status: 0, stdout: JSON.stringify(cloudRunService), stderr: "" };
          }
          assert.deepEqual(args.slice(0, 4), [
            "run",
            "revisions",
            "describe",
            "openagents-monolith-00400-test",
          ]);
          return { status: 0, stdout: JSON.stringify(cloudRunRevision), stderr: "" };
        }
        assert.equal(bin, "psql");
        assert.ok(args.includes("--no-psqlrc"));
        assert.match(args.at(-1), /session\.state IN \('reserved', 'connected'\)/u);
        assert.match(args.at(-1), /credit_mode <> 'owner_waived_unmetered'/u);
        return { status: 0, stdout: counts, stderr: "" };
      },
    }),
  );

test("derives the admission-disable receipt from serving config and drained counts", () => {
  assert.deepEqual(collect(), {
    schemaVersion: "openagents.livekit_admission_disable.v1",
    stage: "production",
    sourceBaseRevision: bundle.sourceBaseRevision,
    deployedRevision: "a".repeat(40),
    observedAt: "2026-08-01T12:00:00.000Z",
    resourceRef: "livekit-admission-ref://production/livekit-room-v1",
    newAdmissionDisabled: true,
    newDispatchDisabled: true,
    activeRoomCount: 0,
    pendingSettlementCount: 0,
  });
});

test("accepts tagged revisions that receive no production traffic", () => {
  const tagged = service();
  tagged.status.traffic.push({
    revisionName: "openagents-monolith-broker-test",
    tag: "broker-test",
  });
  assert.equal(collect(tagged).activeRoomCount, 0);
});

test("refuses admission drift, split traffic, active rooms, and unsettled accounting", () => {
  assert.throws(() => collect(service(), "0,0\n", revision("true")), /admission is not disabled/u);
  const split = service();
  split.status.traffic = [
    { revisionName: split.status.latestReadyRevisionName, percent: 90 },
    { revisionName: "older", percent: 10 },
  ];
  assert.throws(() => collect(split), /traffic is not wholly/u);
  assert.throws(() => collect(service(), "1,0\n"), /active LiveKit rooms/u);
  assert.throws(() => collect(service(), "0,1\n"), /pending LiveKit settlements/u);
});

test("never puts the database credential into psql argv", () => {
  withDatabaseEnvironment(() => {
    collectAdmissionDisableReceipt({
      bundle,
      deployedRevision: "b".repeat(40),
      runCommand(bin, args, environment) {
        if (bin === "gcloud") {
          return {
            status: 0,
            stdout: JSON.stringify(args[1] === "services" ? service() : revision()),
            stderr: "",
          };
        }
        assert.equal(
          args.some((value) => value.includes("not-observed")),
          false,
        );
        assert.equal(environment.PGPASSWORD, "not-observed");
        return { status: 0, stdout: "0,0\n", stderr: "" };
      },
    });
  });
});
