import {
  agentExpectation,
  type AgentAction,
  type AgentCaptureEnvelope,
  type AgentCaptureRequest,
  type AgentDiscovery,
  type AgentExpectation,
  type AgentInspection,
  type AgentInspectOptions,
  type AgentRunLog,
  type AgentStep,
} from "../../packages/runtime/agent-contract.ts";
import {
  verifyAgentCapture,
  verifyCausalEdit,
  verifyQuestTransition,
  OWNED_STARTER_LANDMARKS,
  type OracleCaptureExpectation,
  type OracleCausalExpectation,
  type OracleLandmark,
  type OracleQuestTransition,
} from "../../packages/runtime/agent-oracle.ts";

interface Client {
  discover(): Promise<AgentDiscovery>;
  inspect(options?: AgentInspectOptions): Promise<AgentInspection>;
  execute(action: AgentAction, expect?: AgentExpectation): Promise<AgentStep>;
  capture(request: AgentCaptureRequest): Promise<AgentCaptureEnvelope>;
  exportLog(): AgentRunLog | Promise<AgentRunLog>;
  replay(log: AgentRunLog): Promise<unknown>;
  close(): void | Promise<void>;
}
const client = (): Client =>
  (window as unknown as { aethyrforge: { agent: Client } }).aethyrforge.agent;
const captures = new Map<
  string,
  {
    capture: AgentCaptureEnvelope;
    inspection: AgentInspection;
    expected: OracleCaptureExpectation;
  }
>();
async function digest(pixels: ArrayBuffer) {
  const bytes = new Uint8Array(await crypto.subtle.digest("SHA-256", pixels));
  return [...bytes].map((n) => n.toString(16).padStart(2, "0")).join("");
}
function held(name: string) {
  const value = captures.get(name);
  if (!value) throw Error(`Capture ${name} is unavailable.`);
  return value;
}
export const discover = () => client().discover();
export const inspect = (options?: AgentInspectOptions) =>
  client().inspect(options);
export const execute = (action: AgentAction, expect?: AgentExpectation) =>
  client().execute(action, expect);
export const exportLog = () => client().exportLog();
export const replay = (log: AgentRunLog) => client().replay(log);
export const close = () => client().close();
export { agentExpectation, OWNED_STARTER_LANDMARKS, verifyQuestTransition };
export async function capture(
  name: string,
  request: Omit<AgentCaptureRequest, "expect">,
  landmarks: readonly OracleLandmark[] = [],
  allowFlatEmptyControl = false,
) {
  const before = await inspect();
  const expectation = agentExpectation(before.context);
  const started = performance.now();
  const result = await client().capture({ ...request, expect: expectation });
  const elapsedMs = performance.now() - started;
  // Geometry counts must come from the completed fixed-view GPU submission.
  const inspection = await inspect();
  const expected: OracleCaptureExpectation = {
    ...expectation,
    ...request,
    landmarks,
    worldId: inspection.context.worldId,
    sessionId: inspection.sessionId,
    requestId: result.requestId,
    ...(allowFlatEmptyControl ? { minLumaRange: 0, minLumaStdDev: 0 } : {}),
  };
  const oracle = await verifyAgentCapture(result, expected);
  captures.set(name, { capture: result, inspection, expected });
  const { pixels, ...metadata } = result;
  let encoded = "";
  const bytes = new Uint8Array(pixels);
  for (let i = 0; i < bytes.length; i += 8192)
    encoded += String.fromCharCode(...bytes.subarray(i, i + 8192));
  return { metadata, inspection, oracle, elapsedMs, base64: btoa(encoded) };
}
type CausalInputs = Omit<
  OracleCausalExpectation,
  "beforeInspection" | "afterInspection" | "emptyCapture"
>;
export function causal(
  beforeName: string,
  afterName: string,
  expected: CausalInputs,
  emptyName?: string,
) {
  const before = held(beforeName),
    after = held(afterName);
  return verifyCausalEdit(before.capture, after.capture, {
    ...expected,
    beforeInspection: before.inspection,
    afterInspection: after.inspection,
    ...(emptyName ? { emptyCapture: held(emptyName).capture } : {}),
  });
}
export async function staleCapture(
  expect: AgentExpectation,
  request: Omit<AgentCaptureRequest, "expect">,
) {
  try {
    await client().capture({ ...request, expect });
    return { accepted: true };
  } catch (error) {
    return { accepted: false, message: String(error) };
  }
}
export async function negativeControls(options: {
  empty: string;
  old: string;
  current: string;
  wrongView: string;
  add: CausalInputs;
  transform: CausalInputs;
}) {
  const empty = held(options.empty),
    old = held(options.old),
    current = held(options.current);
  async function forged(pixels: ArrayBuffer, camera = current.capture.camera) {
    return {
      ...structuredClone(current.capture),
      camera: structuredClone(camera),
      pixels: pixels.slice(0),
      pixelSha256: await digest(pixels),
    };
  }
  const black = new Uint8Array(current.capture.pixels.byteLength);
  for (let i = 3; i < black.length; i += 4) black[i] = 255;
  const blackCapture = await forged(black.buffer);
  const missingCapture = await forged(empty.capture.pixels);
  const stalePixels = await forged(old.capture.pixels);
  const wrongCamera = structuredClone(current.capture.camera);
  wrongCamera.eye[0] += 3;
  const wrongCameraCapture = await forged(current.capture.pixels, wrongCamera);
  const wrongPixels = await forged(held(options.wrongView).capture.pixels);
  const add: OracleCausalExpectation = {
    ...options.add,
    beforeInspection: empty.inspection,
    afterInspection: current.inspection,
    emptyCapture: empty.capture,
  };
  const transform: OracleCausalExpectation = {
    ...options.transform,
    beforeInspection: old.inspection,
    afterInspection: current.inspection,
    emptyCapture: empty.capture,
  };
  return {
    hashesRecomputed: true,
    forgedHashes: {
      black: blackCapture.pixelSha256,
      missingGeometry: missingCapture.pixelSha256,
      stalePixels: stalePixels.pixelSha256,
      wrongCamera: wrongCameraCapture.pixelSha256,
      wrongCameraPixels: wrongPixels.pixelSha256,
    },
    black: await verifyAgentCapture(blackCapture, current.expected),
    missingGeometry: verifyCausalEdit(empty.capture, missingCapture, add),
    stalePixels: verifyCausalEdit(old.capture, stalePixels, transform),
    wrongCamera: await verifyAgentCapture(wrongCameraCapture, current.expected),
    wrongCameraPixels: current.expected.landmarks?.length
      ? await verifyAgentCapture(wrongPixels, current.expected)
      : null,
    boundary:
      "Counterfeits retain current context and recomputed pixel hashes. Missing geometry and stale pixels use causal spatial checks; wrong-view pixels require independently specified landmarks.",
  };
}
export function quest(transition: OracleQuestTransition) {
  return verifyQuestTransition(transition);
}
