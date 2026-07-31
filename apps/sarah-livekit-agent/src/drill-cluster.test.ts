import { describe, expect, test } from "vite-plus/test";
import {
  countLiveRooms,
  parseLiveKitSfuGauges,
  selectSoleSfuPodHostingARoom,
  selectSoleWorkerPodForGeneration,
  type LiveKitSfuGauge,
} from "./drill-cluster.js";

/** A trimmed copy of a real `livekit-server` exposition, gauges verbatim. */
const exposition = (rooms: number, participants: number): string =>
  [
    "# HELP livekit_participant_join_total",
    'livekit_participant_join_total{node_id="ND_93ENJzEqD9oX",node_type="SERVER",state="rtc_active"} 33',
    `livekit_participant_total{node_id="ND_93ENJzEqD9oX",node_type="SERVER"} ${participants}`,
    'livekit_room_duration_seconds_count{node_id="ND_93ENJzEqD9oX",node_type="SERVER"} 13',
    `livekit_room_total{node_id="ND_93ENJzEqD9oX",node_type="SERVER"} ${rooms}`,
  ].join("\n");

const gauge = (podName: string, roomTotal: number, participantTotal = 0): LiveKitSfuGauge => ({
  podName,
  roomTotal,
  participantTotal,
});

describe("LiveKit SFU gauge parsing", () => {
  test("reads the two gauges a drill needs and ignores the similarly named ones", () => {
    expect(parseLiveKitSfuGauges("livekit-server-a", exposition(1, 3))).toEqual({
      podName: "livekit-server-a",
      roomTotal: 1,
      participantTotal: 3,
    });
  });

  test("fails closed when a gauge is absent rather than defaulting it to zero", () => {
    // A zero default would silently turn "I could not read the metric" into
    // "this instance hosts no room", which is how a drill destroys the wrong pod.
    expect(() => parseLiveKitSfuGauges("livekit-server-a", "# nothing here")).toThrow(
      "livekit_room_total is missing",
    );
    expect(() =>
      parseLiveKitSfuGauges(
        "livekit-server-a",
        'livekit_room_total{node_id="ND_x",node_type="SERVER"} 1',
      ),
    ).toThrow("livekit_participant_total is missing");
  });
});

describe("SFU fault target selection", () => {
  test("names the single instance hosting a room", () => {
    expect(
      selectSoleSfuPodHostingARoom([
        gauge("livekit-server-a", 0),
        gauge("livekit-server-b", 1, 3),
        gauge("livekit-server-c", 0),
      ]).podName,
    ).toBe("livekit-server-b");
  });

  test("refuses when a second room is live rather than guessing a target", () => {
    expect(() =>
      selectSoleSfuPodHostingARoom([gauge("livekit-server-a", 1), gauge("livekit-server-b", 1)]),
    ).toThrow("not attributable");
  });

  test("refuses when the drill session is not live at the SFU", () => {
    expect(() =>
      selectSoleSfuPodHostingARoom([gauge("livekit-server-a", 0), gauge("livekit-server-b", 0)]),
    ).toThrow("not live at the SFU");
    expect(() => selectSoleSfuPodHostingARoom([])).toThrow("no livekit-server instance reported");
  });

  test("counts cluster-wide rooms as the conservative billable-session proxy", () => {
    expect(countLiveRooms([gauge("a", 0), gauge("b", 1), gauge("c", 0)])).toBe(1);
    expect(countLiveRooms([gauge("a", 2), gauge("b", 1)])).toBe(3);
    expect(countLiveRooms([])).toBe(0);
  });
});

describe("Sarah worker fault target selection", () => {
  const logs = (...entries: readonly (readonly [string, string])[]) =>
    entries.map(([podName, log]) => ({ podName, log }));

  test("names the single worker that logged the drill participant", () => {
    expect(
      selectSoleWorkerPodForGeneration(
        logs(
          ["sarah-a", '{"participantValue":"owner-000000"}'],
          ["sarah-b", '{"participantValue":"owner-2f9ab1"}'],
          ["sarah-c", "idle"],
        ),
        "owner-2f9ab1",
      ),
    ).toBe("sarah-b");
  });

  test("refuses when the job was never accepted", () => {
    expect(() =>
      selectSoleWorkerPodForGeneration(logs(["sarah-a", "idle"]), "owner-2f9ab1"),
    ).toThrow("the job was not accepted");
  });

  test("refuses when two workers logged one generation", () => {
    // The failure matrix already forbids overlapping worker generations, so a
    // second match is a finding, not an ambiguity to resolve by picking one.
    expect(() =>
      selectSoleWorkerPodForGeneration(
        logs(
          ["sarah-a", '{"participantValue":"owner-2f9ab1"}'],
          ["sarah-b", '{"participantValue":"owner-2f9ab1"}'],
        ),
        "owner-2f9ab1",
      ),
    ).toThrow("handled twice");
  });

  test("refuses a blank participant ref, which would match every log", () => {
    expect(() => selectSoleWorkerPodForGeneration(logs(["sarah-a", "anything"]), "  ")).toThrow(
      "needs the drill participant ref",
    );
  });
});
