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
 * The single Sarah worker that accepted the drill's generation.
 *
 * The discriminator is the owner PARTICIPANT ref, not the room ref. The worker
 * log carries `jobId`, `agentName`, and `participantValue`, and it never prints
 * the room name — verified against the live production workers on 2026-07-31 —
 * so a room-ref scan matches nothing and would make every worker-targeting drill
 * unrunnable for a reason that reads as "the job was not accepted".
 *
 * The participant ref is `owner-<sha256(ownerUserId) truncated>`, so it is
 * stable per OWNER and not per generation. That makes the log WINDOW
 * load-bearing: the caller must read only logs since this session started, or an
 * earlier generation by the same owner — an abandoned attempt minutes ago, say —
 * matches on a different pod and this selector refuses a perfectly good drill as
 * "handled twice". Within a correct window, two matches really would mean one
 * generation was handled twice, which the failure matrix already forbids
 * (`maximumWorkerGenerationCount: 1`) and which no drill should paper over.
 */
export const selectSoleWorkerPodForGeneration = (
  logs: readonly Readonly<{ podName: string; log: string }>[],
  participantRef: string,
): string => {
  if (participantRef.trim() === "") {
    throw new Error("worker discovery needs the drill participant ref");
  }
  const matches = logs.filter((entry) => entry.log.includes(participantRef));
  if (matches.length === 0) {
    throw new Error("no Sarah worker logged the drill participant: the job was not accepted");
  }
  if (matches.length > 1) {
    throw new Error(
      `${matches.length} Sarah workers logged the drill participant, so one generation was ` +
        "handled twice",
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

/**
 * The Managed Prometheus substitution for the gauge sweep.
 *
 * `readLiveKitSfuGauges` is the runbook's verbatim procedure and needs
 * `pods/exec`. That verb is deliberately NOT granted to the drill automation
 * identity — the runbook records it as excluded on purpose, because exec is
 * arbitrary command execution inside a production pod — and a real API call on
 * 2026-07-31 confirmed `Forbidden` for `oa-mvp-automation@` once the shared
 * `gke-gcloud-auth-plugin` cache was cleared. An earlier receipt claimed exec
 * "is granted in practice"; that claim was read through a cache poisoned by the
 * owner identity and is wrong for the identity a drill actually runs as.
 *
 * The runbook already names this substitution and states exactly what made it
 * insufficient: the `PodMonitoring` scrape interval is 30 s while an acceptance
 * session lived about 21 s, so the gauge could not identify the instance
 * carrying a room inside the session it was meant to target — "until either the
 * drill runs against a held-open session or the scrape interval is lowered".
 * The single-session driver holds the session open, so that condition is now
 * satisfied by the driver rather than by widening anyone's authority.
 *
 * Two properties keep this honest:
 *
 *   - Only the LATEST point per instance is read, never the maximum over the
 *     window. A maximum would let a room that ended a minute ago name a second
 *     "hosting" instance and turn a good drill into a false ambiguity refusal —
 *     or worse, aim a fault at an instance that no longer carries the room.
 *   - Every running instance must have produced at least one sample in the
 *     window. A missing series is a read this function refuses to complete,
 *     not an instance silently counted as hosting zero rooms.
 *
 * The cost is staleness: a sample is up to one scrape interval plus ingestion
 * latency old (measured at about 44 s worst case). `MANAGED_PROMETHEUS_MINIMUM_HOLD_MS`
 * is the hold this bounds, and the CLI refuses a shorter one.
 */
export const MANAGED_PROMETHEUS_PROJECT = "openagentsgemini";
export const MANAGED_PROMETHEUS_LOOKBACK_MS = 300_000;
/**
 * The shortest hold that guarantees a scrape of the drill's own room has been
 * ingested before the gauge is read: one 30 s scrape interval, plus the ~14 s
 * ingestion latency measured on 2026-07-31, plus margin.
 */
export const MANAGED_PROMETHEUS_MINIMUM_HOLD_MS = 90_000;

export type ManagedPrometheusPoint = Readonly<{
  interval: Readonly<{ endTime: string }>;
  value: Readonly<{ doubleValue?: number; int64Value?: string }>;
}>;

export type ManagedPrometheusSeries = Readonly<{
  metric: Readonly<{ labels?: Readonly<{ pod?: string }> }>;
  points?: readonly ManagedPrometheusPoint[];
}>;

const pointValue = (point: ManagedPrometheusPoint): number => {
  const value = point.value.doubleValue ?? Number(point.value.int64Value);
  if (!Number.isFinite(value)) {
    throw new Error("managed prometheus returned a nonnumeric gauge sample");
  }
  return value;
};

/**
 * The latest sample per instance for one gauge, keyed by pod name.
 *
 * Series are returned per instance, so "latest" is resolved within each series
 * rather than across them.
 */
const latestByPod = (series: readonly ManagedPrometheusSeries[]): ReadonlyMap<string, number> => {
  const latest = new Map<string, { atMs: number; value: number }>();
  for (const entry of series) {
    const podName = entry.metric.labels?.pod;
    if (podName === undefined || podName === "") continue;
    for (const point of entry.points ?? []) {
      const atMs = Date.parse(point.interval.endTime);
      if (!Number.isFinite(atMs)) continue;
      const held = latest.get(podName);
      if (held === undefined || atMs > held.atMs) {
        latest.set(podName, { atMs, value: pointValue(point) });
      }
    }
  }
  return new Map([...latest].map(([podName, held]) => [podName, held.value]));
};

/**
 * Project two Managed Prometheus responses onto the gauge shape the selectors
 * consume.
 *
 * Separated from the network call because this is the part that decides which
 * instance a fault is aimed at, and it must be checkable without a cluster.
 * Two rules carry the weight, and both are about failing closed:
 *
 *   - The LATEST sample per instance wins, never the maximum over the window. A
 *     maximum would let a room that ended a minute ago keep naming its old host
 *     as "hosting", which either refuses a good drill for false ambiguity or
 *     aims a deletion at an instance that no longer carries the room.
 *   - A running instance with no sample in the window is a refusal, not a zero.
 *     Treating an unread instance as hosting nothing is exactly how the one pod
 *     that does carry the room gets skipped and a healthy one gets destroyed.
 */
export const selectLatestManagedPrometheusGauges = (
  pods: readonly string[],
  roomSeries: readonly ManagedPrometheusSeries[],
  participantSeries: readonly ManagedPrometheusSeries[],
): readonly LiveKitSfuGauge[] => {
  const rooms = latestByPod(roomSeries);
  const participants = latestByPod(participantSeries);
  return pods.map((podName) => {
    const roomTotal = rooms.get(podName);
    const participantTotal = participants.get(podName);
    if (roomTotal === undefined || participantTotal === undefined) {
      throw new Error(
        `managed prometheus has no recent gauge sample for livekit-server instance ${podName}, ` +
          "so the cluster reading is incomplete",
      );
    }
    return { podName, roomTotal, participantTotal };
  });
};

const managedPrometheusGauge = async (
  accessToken: string,
  metricName: string,
  startTime: string,
  endTime: string,
): Promise<readonly ManagedPrometheusSeries[]> => {
  const url = new URL(
    `https://monitoring.googleapis.com/v3/projects/${MANAGED_PROMETHEUS_PROJECT}/timeSeries`,
  );
  url.searchParams.set(
    "filter",
    `metric.type="prometheus.googleapis.com/${metricName}/gauge" AND ` +
      `resource.labels.namespace="${LIVEKIT_NAMESPACE}"`,
  );
  url.searchParams.set("interval.startTime", startTime);
  url.searchParams.set("interval.endTime", endTime);
  const response = await fetch(url, { headers: { authorization: `Bearer ${accessToken}` } });
  if (!response.ok) {
    // The status, never the body: an error body from the monitoring API can
    // echo the request, and the request carries a bearer token.
    throw new Error(`managed prometheus read for ${metricName} failed with ${response.status}`);
  }
  const body = (await response.json()) as Readonly<{
    timeSeries?: readonly ManagedPrometheusSeries[];
  }>;
  return body.timeSeries ?? [];
};

const managedPrometheusAccessToken = async (): Promise<string> => {
  const { stdout } = await run_("gcloud", ["auth", "print-access-token"]);
  const token = stdout.trim();
  if (token === "")
    throw new Error("gcloud produced no access token for the managed prometheus read");
  return token;
};

export const readLiveKitSfuGaugesFromManagedPrometheus: ReadLiveKitSfuGauges = async () => {
  const pods = await podNames(SFU_SELECTOR);
  if (pods.length === 0) throw new Error("no running livekit-server instance to read gauges for");
  const nowMs = Date.now();
  const startTime = new Date(nowMs - MANAGED_PROMETHEUS_LOOKBACK_MS).toISOString();
  const endTime = new Date(nowMs).toISOString();
  const accessToken = await managedPrometheusAccessToken();
  const rooms = await managedPrometheusGauge(accessToken, "livekit_room_total", startTime, endTime);
  const participants = await managedPrometheusGauge(
    accessToken,
    "livekit_participant_total",
    startTime,
    endTime,
  );
  return selectLatestManagedPrometheusGauges(pods, rooms, participants);
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
