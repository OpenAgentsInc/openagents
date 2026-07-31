import { execFile } from "node:child_process";
import { promisify } from "node:util";

/**
 * Fault-target discovery for the single-session drills.
 *
 * A drill cannot be told its target in advance. The SFU instance carrying a
 * room is chosen by the cluster when the room is created, and the Sarah worker
 * that accepts a job is chosen by LiveKit dispatch, so both are only knowable
 * once the session is live — which is exactly when the driver is holding it
 * open.
 *
 * Every selector here fails closed on ambiguity. A drill that destroys the
 * wrong instance is worse than a drill that does not run: it takes a session
 * the drill has no claim over and produces an observation nothing can be
 * attributed to. "Two candidates" and "no candidate" are both refusals, never a
 * guess.
 *
 * Discovery is read-only. Nothing in this module deletes anything.
 */

const run_ = promisify(execFile);

export const LIVEKIT_NAMESPACE = "livekit-system";
export const SFU_SELECTOR = "app.kubernetes.io/name=livekit-server";
export const WORKER_SELECTOR = "app.kubernetes.io/name=sarah-livekit-agent";
/** The loopback port `livekit-server` publishes its Prometheus gauges on. */
export const SFU_METRICS_PORT = 6789;

export type LiveKitSfuGauge = Readonly<{
  podName: string;
  /** `livekit_room_total`: rooms this instance is currently hosting. */
  roomTotal: number;
  /** `livekit_participant_total`: participants currently connected to it. */
  participantTotal: number;
}>;

export type ReadLiveKitSfuGauges = () => Promise<readonly LiveKitSfuGauge[]>;
export type ReadSarahWorkerLogs = () => Promise<
  readonly Readonly<{ podName: string; log: string }>[]
>;

const gaugePattern = (name: string): RegExp => new RegExp(`^${name}\\{[^}]*\\}\\s+(\\d+)`, "mu");

/** Parse one Prometheus text exposition for the two gauges a drill needs. */
export const parseLiveKitSfuGauges = (podName: string, exposition: string): LiveKitSfuGauge => {
  const read = (name: string): number => {
    const matched = gaugePattern(name).exec(exposition);
    if (matched?.[1] === undefined) {
      throw new Error(`${name} is missing from the livekit-server metrics exposition`);
    }
    return Number(matched[1]);
  };
  return {
    podName,
    roomTotal: read("livekit_room_total"),
    participantTotal: read("livekit_participant_total"),
  };
};

/**
 * The single SFU instance hosting the drill's room.
 *
 * With one room in the cluster, exactly one instance reports a nonzero
 * `livekit_room_total`. That is what makes the target identifiable without
 * reading a room name off the wire, and it is the same property that makes the
 * fault non-customer-affecting: a second nonzero instance means a second room
 * exists, so the fault would take a session this drill has no claim over.
 */
export const selectSoleSfuPodHostingARoom = (
  gauges: readonly LiveKitSfuGauge[],
): LiveKitSfuGauge => {
  if (gauges.length === 0) throw new Error("no livekit-server instance reported metrics");
  const hosting = gauges.filter((gauge) => gauge.roomTotal > 0);
  if (hosting.length === 0) {
    throw new Error(
      "no livekit-server instance is hosting a room: the drill session is not live at the SFU",
    );
  }
  if (hosting.length > 1) {
    throw new Error(
      `${hosting.length} livekit-server instances are hosting rooms, so the drill target is not ` +
        "attributable and the fault would take a session the drill has no claim over",
    );
  }
  return hosting[0] as LiveKitSfuGauge;
};

/**
 * Rooms live anywhere in the cluster.
 *
 * A Sarah voice generation creates exactly one room, and a room can exist
 * without being a billable Sarah generation, so this over-counts. Over-counting
 * is the safe direction: a measured one means at most one billable session, so
 * the `sfu_loss` precondition is measured rather than attested.
 */
export const countLiveRooms = (gauges: readonly LiveKitSfuGauge[]): number =>
  gauges.reduce((total, gauge) => total + gauge.roomTotal, 0);

/**
 * The single Sarah worker that accepted the drill's job.
 *
 * The room ref appears in the accepting worker's log and nowhere else, so a
 * scan across the worker pods names one instance. Two matches would mean one
 * generation was handled twice, which the failure matrix already forbids
 * (`maximumWorkerGenerationCount: 1`) and which no drill should paper over.
 */
export const selectSoleWorkerPodForRoom = (
  logs: readonly Readonly<{ podName: string; log: string }>[],
  roomRef: string,
): string => {
  if (roomRef.trim() === "") throw new Error("worker discovery needs the drill room ref");
  const matches = logs.filter((entry) => entry.log.includes(roomRef));
  if (matches.length === 0) {
    throw new Error("no Sarah worker logged the drill room: the job was not accepted");
  }
  if (matches.length > 1) {
    throw new Error(
      `${matches.length} Sarah workers logged the drill room, so one generation was handled twice`,
    );
  }
  return (matches[0] as { podName: string }).podName;
};

const podNames = async (selector: string): Promise<readonly string[]> => {
  const { stdout } = await run_("kubectl", [
    "get",
    "pods",
    "--namespace",
    LIVEKIT_NAMESPACE,
    "--selector",
    selector,
    "--field-selector",
    "status.phase=Running",
    "--output",
    'jsonpath={range .items[*]}{.metadata.name}{"\\n"}{end}',
  ]);
  return stdout
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line !== "");
};

/**
 * Read every SFU instance's room and participant gauges.
 *
 * This is the runbook's own procedure — `kubectl exec` plus a loopback `wget`
 * of the metrics port — rather than the Managed Prometheus alternative, because
 * that alternative scrapes every thirty seconds against a session that lives
 * about twenty. A gauge read that can be a scrape interval stale cannot name
 * the instance carrying a room right now.
 */
export const readLiveKitSfuGauges: ReadLiveKitSfuGauges = async () => {
  const pods = await podNames(SFU_SELECTOR);
  const gauges: LiveKitSfuGauge[] = [];
  for (const podName of pods) {
    // Sequential: a concurrent sweep gives no ordering over a gauge that is
    // changing, and a failed read has to name the pod it failed on.
    // eslint-disable-next-line no-await-in-loop
    const { stdout } = await run_(
      "kubectl",
      [
        "exec",
        "--namespace",
        LIVEKIT_NAMESPACE,
        podName,
        "--",
        "wget",
        "-qO-",
        `http://127.0.0.1:${SFU_METRICS_PORT}/metrics`,
      ],
      { maxBuffer: 16 * 1024 * 1024 },
    );
    gauges.push(parseLiveKitSfuGauges(podName, stdout));
  }
  return gauges;
};

export const readSarahWorkerLogs = async (
  sinceSeconds = 900,
): Promise<readonly Readonly<{ podName: string; log: string }>[]> => {
  const pods = await podNames(WORKER_SELECTOR);
  const logs: Array<Readonly<{ podName: string; log: string }>> = [];
  for (const podName of pods) {
    // eslint-disable-next-line no-await-in-loop
    const { stdout } = await run_(
      "kubectl",
      [
        "logs",
        "--namespace",
        LIVEKIT_NAMESPACE,
        podName,
        `--since=${sinceSeconds}s`,
        "--tail=4000",
      ],
      { maxBuffer: 32 * 1024 * 1024 },
    );
    logs.push({ podName, log: stdout });
  }
  return logs;
};

const POD_NAME = /^[a-z0-9]([a-z0-9-]{0,251}[a-z0-9])?$/u;

/**
 * Destroy exactly one named pod.
 *
 * `--grace-period=0 --force` is what makes this loss rather than a drain: a
 * graceful termination lets the instance hand its rooms over, which is a
 * different drill and is not the fault this row is about. `execFile` with an
 * argument array, never a shell, so no discovered name can widen the action.
 */
export const deleteExactPod = async (podName: string): Promise<void> => {
  if (!POD_NAME.test(podName)) throw new Error("pod name is not a Kubernetes object name");
  await run_("kubectl", [
    "delete",
    "pod",
    "--namespace",
    LIVEKIT_NAMESPACE,
    podName,
    "--grace-period=0",
    "--force",
  ]);
};

export const podIsRunning = async (podName: string): Promise<boolean> => {
  if (!POD_NAME.test(podName)) throw new Error("pod name is not a Kubernetes object name");
  try {
    const { stdout } = await run_("kubectl", [
      "get",
      "pod",
      "--namespace",
      LIVEKIT_NAMESPACE,
      podName,
      "--output",
      "jsonpath={.status.phase}",
    ]);
    return stdout.trim() === "Running";
  } catch {
    return false;
  }
};
