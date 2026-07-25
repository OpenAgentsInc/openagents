import { Effect, Stream } from "@effect-native/core/effect";
import { readFileSync } from "node:fs";
import { decodeIssue31HostAdjunct } from "@openagentsinc/sarah/issue31-workroom";
import { describe, expect, test } from "vite-plus/test";

import {
  ISSUE31_CAPABILITY_DESCRIPTORS,
  decodeIssue31SourceSnapshot,
  issue31RowsForRoom,
  projectIssue31WorkroomReadModel,
  type Issue31SourceSnapshot,
} from "../src/workroom/issue31-workroom-read-model";
import {
  buildHomeProgram,
  chromeProps,
  renderContentView,
  renderDrawerView,
} from "../src/screens/home-core";

const observedAt = "2026-07-24T16:00:00.000Z";
const eventRef = "a".repeat(64);

const readySources = (): ReadonlyArray<Issue31SourceSnapshot> =>
  ISSUE31_CAPABILITY_DESCRIPTORS.map((descriptor) =>
    decodeIssue31SourceSnapshot({
      capabilityId: descriptor.id,
      authority: descriptor.expectedAuthority,
      sourceRef: descriptor.sourceRef,
      status: "ready",
      freshness: "live",
      observedAt,
      recordRefs:
        descriptor.expectedAuthority === "signed_nostr_record"
          ? [eventRef]
          : [`projection.issue31.${descriptor.id}`],
      reasonRef: null,
      role: descriptor.room === "community" ? "member" : "owner",
      roleStatus: "active",
      actionState:
        descriptor.id === "full_auto"
          ? {
              kind: "terminal",
              intentRef: "intent.full-auto.stop.test",
              actionRef: "action.full-auto.stop",
              state: "stopped",
              outcomeRef: "outcome.full-auto.stop.test",
            }
          : { kind: "idle" },
    }),
  );

const settle = Effect.gen(function* () {
  yield* Effect.yieldNow;
  yield* Effect.yieldNow;
});

const lastState = (program: ReturnType<typeof buildHomeProgram>) =>
  Effect.map(Stream.runHead(program.stateChanges), (option) => {
    if (option._tag !== "Some") throw new Error("expected state");
    return option.value;
  });

describe("Issue31WorkroomReadModel", () => {
  test("freezes one honest row for every issue 31 capability", () => {
    const model = projectIssue31WorkroomReadModel({ projectedAt: observedAt });
    expect(model.rows.map((row) => row.id)).toEqual(
      ISSUE31_CAPABILITY_DESCRIPTORS.map((row) => row.id),
    );
    expect(model.coverage).toEqual({
      total: 11,
      ready: 0,
      unavailable: 11,
      gaps: 0,
      pending: 0,
      refused: 0,
      terminal: 0,
    });
    expect(
      model.rows.every(
        (row) =>
          row.source.status === "unavailable" &&
          row.source.reasonRef === `reason.issue31.source_not_connected:${row.id}`,
      ),
    ).toBe(true);
  });

  test("accepts only the frozen authority and source identity for each row", () => {
    const model = projectIssue31WorkroomReadModel({
      projectedAt: observedAt,
      sources: readySources(),
    });
    expect(model.coverage.ready).toBe(11);
    expect(model.coverage.terminal).toBe(1);
    expect(model.rows.filter((row) => row.source.authority === "signed_nostr_record")).toHaveLength(
      8,
    );
    expect(model.rows.filter((row) => row.source.authority === "omega_host_adjunct")).toHaveLength(
      3,
    );

    expect(() =>
      decodeIssue31SourceSnapshot({
        ...readySources()[0],
        authority: "omega_host_adjunct",
      }),
    ).toThrow(/source identity/);
    expect(() =>
      decodeIssue31SourceSnapshot({
        ...readySources()[0],
        recordRefs: [],
      }),
    ).toThrow(/authority record/);
    expect(() =>
      decodeIssue31SourceSnapshot({
        ...readySources()[0],
        recordRefs: ["file:///Users/owner/private"],
      }),
    ).toThrow();
    expect(() =>
      projectIssue31WorkroomReadModel({
        projectedAt: observedAt,
        sources: [readySources()[0], readySources()[0]],
      }),
    ).toThrow(/more than once/);
  });

  test("keeps owner-private and community records in distinct room projections", () => {
    const model = projectIssue31WorkroomReadModel({
      projectedAt: observedAt,
      sources: readySources(),
    });
    const owner = issue31RowsForRoom(model, "owner_private");
    const community = issue31RowsForRoom(model, "community");
    expect(owner.map((row) => row.id)).not.toContain("community_work");
    expect(community.map((row) => row.id)).not.toContain("owner_private_sarah");
    expect(owner.filter((row) => row.room === "shared").map((row) => row.id)).toEqual(
      community.filter((row) => row.room === "shared").map((row) => row.id),
    );
  });

  test("joins host-only state without replacing signed Nostr connection authority", () => {
    const hostAdjunct = decodeIssue31HostAdjunct(
      JSON.parse(
        readFileSync(
          new URL(
            "../../../packages/sarah/fixtures/issue31-workroom/openagents.omega.issue31.host.v1.canonical.json",
            import.meta.url,
          ),
          "utf8",
        ),
      ),
    );
    const model = projectIssue31WorkroomReadModel({
      projectedAt: observedAt,
      hostAdjunct,
    });
    const connection = model.rows.find((row) => row.id === "connection_and_identity");
    expect(connection?.source.authority).toBe("signed_nostr_record");
    expect(connection?.source.status).toBe("unavailable");
    expect(connection?.hostObservation?.projection.capability).toBe("connection_identity");
    expect(model.rows.find((row) => row.id === "full_auto")?.source.status).toBe("gap");
    expect(model.rows.find((row) => row.id === "full_auto")?.source.actionState.kind).toBe(
      "pending",
    );
    expect(model.rows.find((row) => row.id === "provider_accounts")?.source.status).toBe("ready");
    expect(model.rows.find((row) => row.id === "provider_accounts")?.source.actionState.kind).toBe(
      "refused",
    );
    expect(model.rows.find((row) => row.id === "evidence_chain")?.source.status).toBe("ready");
    expect(model.rows.find((row) => row.id === "evidence_chain")?.source.actionState.kind).toBe(
      "terminal",
    );
    expect(model.coverage).toMatchObject({ pending: 1, refused: 1, terminal: 1 });
  });

  test("opens the Workroom route, exposes all missing sources, and switches isolated rooms", async () => {
    const program = buildHomeProgram();
    expect(program.initialState.syncPhase).toBe("unconfigured");
    expect(JSON.stringify(renderDrawerView(program.initialState))).toContain("Workroom");
    program.workroom.open();
    await Effect.runPromise(settle);
    let state = await Effect.runPromise(lastState(program));
    expect(state.workbenchRoute).toBe("workroom");
    expect(chromeProps(state).glassComposerVisible).toBe(false);
    let view = JSON.stringify(renderContentView(state));
    expect(view).toContain("Connection and identity");
    expect(view).toContain("Owner-private Sarah");
    expect(view).not.toContain("Community work");
    expect(view).toContain("reason.issue31.source_not_connected:full_auto");

    program.workroom.selectRoom("community");
    await Effect.runPromise(settle);
    state = await Effect.runPromise(lastState(program));
    view = JSON.stringify(renderContentView(state));
    expect(view).toContain("Community membership");
    expect(view).toContain("Community work");
    expect(view).toContain("v1 awards experience and pays no money");
    expect(view).not.toContain("issue31-capability-owner_private_sarah");
  });
});
