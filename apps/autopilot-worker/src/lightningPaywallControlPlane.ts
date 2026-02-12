import { sha256IdFromString } from "./hash";

export type LightningPaywallToolName =
  | "lightning_paywall_create"
  | "lightning_paywall_update"
  | "lightning_paywall_pause"
  | "lightning_paywall_resume"
  | "lightning_paywall_get"
  | "lightning_paywall_list"
  | "lightning_paywall_settlement_list";

type PaywallStatus = "active" | "paused" | "archived";

type PaywallSummary = {
  readonly paywallId: string;
  readonly ownerId: string | null;
  readonly name: string | null;
  readonly status: PaywallStatus | null;
  readonly fixedAmountMsats: number | null;
  readonly routeCount: number;
};

type SettlementSummary = {
  readonly settlementId: string;
  readonly paywallId: string | null;
  readonly amountMsats: number | null;
  readonly paymentProofRef: string | null;
  readonly createdAtMs: number | null;
};

type PaywallDenyCode =
  | "not_configured"
  | "not_authorized"
  | "forbidden"
  | "not_found"
  | "invalid_input"
  | "invalid_route"
  | "paused"
  | "over_cap"
  | "policy_violation"
  | "rate_limited"
  | "unknown_denial";

export type PaywallToolSideEffect = {
  readonly kind: "http_request";
  readonly target: string;
  readonly method: string;
  readonly status_code: number | null;
  readonly changed: boolean | null;
  readonly detail: string | null;
};

export type LightningPaywallToolOutput = {
  readonly status: "ok" | "denied" | "error";
  readonly denyCode: PaywallDenyCode | null;
  readonly denyReason: string | null;
  readonly errorCode: string | null;
  readonly errorMessage: string | null;
  readonly httpStatus: number | null;
  readonly requestId: string | null;
  readonly paywall: PaywallSummary | null;
  readonly paywalls: ReadonlyArray<PaywallSummary>;
  readonly settlements: ReadonlyArray<SettlementSummary>;
  readonly nextCursor: number | null;
  readonly receipt: {
    readonly params_hash: string;
    readonly output_hash: string;
    readonly latency_ms: number;
    readonly side_effects: ReadonlyArray<PaywallToolSideEffect>;
  };
};

export type ExecuteLightningPaywallToolOptions = {
  readonly toolName: LightningPaywallToolName;
  readonly input: Record<string, unknown>;
  readonly env: unknown;
  readonly fetchImpl?: typeof fetch;
};

type RequestPlan = {
  readonly method: "GET" | "POST" | "PATCH";
  readonly path: string;
  readonly query: ReadonlyArray<readonly [string, string]>;
  readonly body: unknown | undefined;
  readonly changedHint: boolean | null;
};

const asRecord = (value: unknown): Record<string, unknown> | null =>
  value !== null && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;

const asString = (value: unknown): string | null =>
  typeof value === "string" ? value : null;

const asNonEmptyString = (value: unknown): string | null => {
  const text = asString(value);
  if (!text) return null;
  const trimmed = text.trim();
  return trimmed.length > 0 ? trimmed : null;
};

const asNumber = (value: unknown): number | null =>
  typeof value === "number" && Number.isFinite(value) ? value : null;

const asBoolean = (value: unknown): boolean | null =>
  typeof value === "boolean" ? value : null;

const asPaywallStatus = (value: unknown): PaywallStatus | null => {
  if (value === "active" || value === "paused" || value === "archived") return value;
  return null;
};

const stableValue = (value: unknown): unknown => {
  if (Array.isArray(value)) {
    return value.map((item) => stableValue(item));
  }

  const rec = asRecord(value);
  if (rec) {
    const out: Record<string, unknown> = {};
    const keys = Object.keys(rec).sort((a, b) => a.localeCompare(b));
    for (const key of keys) {
      const next = stableValue(rec[key]);
      if (next !== undefined) out[key] = next;
    }
    return out;
  }

  if (typeof value === "number" && !Number.isFinite(value)) {
    return null;
  }

  return value;
};

const stableJson = (value: unknown): string => JSON.stringify(stableValue(value));

const parseJson = (input: string): unknown | null => {
  if (!input.trim()) return null;
  try {
    return JSON.parse(input);
  } catch {
    return null;
  }
};

const normalizeUrlBase = (baseUrl: string): string => baseUrl.replace(/\/+$/, "");

const readEnvString = (env: unknown, keys: ReadonlyArray<string>): string | null => {
  const rec = asRecord(env);
  if (!rec) return null;

  for (const key of keys) {
    const text = asNonEmptyString(rec[key]);
    if (text) return text;
  }

  return null;
};

const controlPlaneConfig = (env: unknown): { readonly baseUrl: string | null; readonly authToken: string | null; readonly timeoutMs: number } => {
  const baseUrl = readEnvString(env, [
    "LIGHTNING_CONTROL_PLANE_BASE_URL",
    "OA_LIGHTNING_CONTROL_PLANE_BASE_URL",
  ]);
  const authToken = readEnvString(env, [
    "LIGHTNING_CONTROL_PLANE_AUTH_TOKEN",
    "OA_LIGHTNING_CONTROL_PLANE_AUTH_TOKEN",
  ]);
  const timeoutRaw = readEnvString(env, ["LIGHTNING_CONTROL_PLANE_TIMEOUT_MS"]);
  const parsedTimeout = timeoutRaw ? Number(timeoutRaw) : NaN;
  const timeoutMs =
    Number.isFinite(parsedTimeout) && parsedTimeout > 0
      ? Math.max(500, Math.min(30_000, Math.floor(parsedTimeout)))
      : 10_000;

  return {
    baseUrl: baseUrl ? normalizeUrlBase(baseUrl) : null,
    authToken,
    timeoutMs,
  };
};

const defaultOutputState = (): Omit<LightningPaywallToolOutput, "receipt"> => ({
  status: "error",
  denyCode: null,
  denyReason: null,
  errorCode: null,
  errorMessage: null,
  httpStatus: null,
  requestId: null,
  paywall: null,
  paywalls: [],
  settlements: [],
  nextCursor: null,
});

const parsePaywallSummary = (value: unknown): PaywallSummary | null => {
  const rec = asRecord(value);
  if (!rec) return null;

  const paywallId = asNonEmptyString(rec.paywallId);
  if (!paywallId) return null;

  const policy = asRecord(rec.policy);
  const routes = Array.isArray(rec.routes) ? rec.routes : [];

  return {
    paywallId,
    ownerId: asNonEmptyString(rec.ownerId),
    name: asNonEmptyString(rec.name),
    status: asPaywallStatus(rec.status),
    fixedAmountMsats: policy ? asNumber(policy.fixedAmountMsats) : null,
    routeCount: routes.length,
  };
};

const sortPaywalls = (rows: ReadonlyArray<PaywallSummary>): ReadonlyArray<PaywallSummary> =>
  [...rows].sort((a, b) => a.paywallId.localeCompare(b.paywallId));

const parseSettlementSummary = (value: unknown): SettlementSummary | null => {
  const rec = asRecord(value);
  if (!rec) return null;

  const settlementId = asNonEmptyString(rec.settlementId);
  if (!settlementId) return null;

  return {
    settlementId,
    paywallId: asNonEmptyString(rec.paywallId),
    amountMsats: asNumber(rec.amountMsats),
    paymentProofRef: asNonEmptyString(rec.paymentProofRef),
    createdAtMs: asNumber(rec.createdAtMs),
  };
};

const sortSettlements = (rows: ReadonlyArray<SettlementSummary>): ReadonlyArray<SettlementSummary> =>
  [...rows].sort((a, b) => {
    const aCreated = a.createdAtMs ?? 0;
    const bCreated = b.createdAtMs ?? 0;
    if (aCreated !== bCreated) return bCreated - aCreated;
    return a.settlementId.localeCompare(b.settlementId);
  });

const extractErrorMessage = (body: unknown, status: number): string => {
  const rec = asRecord(body);
  const explicit = asNonEmptyString(rec?.error) ?? asNonEmptyString(rec?.message);
  if (explicit) return explicit;
  return `upstream_http_${status}`;
};

const classifyDenyCode = (status: number, message: string): PaywallDenyCode => {
  const lower = message.toLowerCase();

  if (lower.includes("over_cap") || lower.includes("quota") || status === 429) {
    return "over_cap";
  }
  if (lower.includes("paused") || lower.includes("inactive")) return "paused";
  if (lower.includes("invalid_route") || lower.includes("route_conflict")) {
    return "invalid_route";
  }
  if (lower.includes("policy_violation")) return "policy_violation";
  if (status === 401 || lower.includes("unauthorized")) return "not_authorized";
  if (status === 403 || lower.includes("forbidden")) return "forbidden";
  if (status === 404 || lower.includes("not_found")) return "not_found";
  if (status === 400 || lower.includes("invalid_input")) return "invalid_input";
  return "unknown_denial";
};

const isDeniedStatus = (status: number): boolean =>
  status === 400 ||
  status === 401 ||
  status === 403 ||
  status === 404 ||
  status === 409 ||
  status === 422 ||
  status === 429;

const buildRequestPlan = (
  toolName: LightningPaywallToolName,
  input: Record<string, unknown>,
): RequestPlan => {
  const paywallId = asNonEmptyString(input.paywallId) ?? "";

  switch (toolName) {
    case "lightning_paywall_create":
      return {
        method: "POST",
        path: "/api/lightning/paywalls",
        query: [],
        body: {
          name: input.name,
          description: input.description,
          status: input.status,
          policy: input.policy,
          routes: input.routes,
          metadata: input.metadata,
        },
        changedHint: true,
      };

    case "lightning_paywall_update":
      return {
        method: "PATCH",
        path: `/api/lightning/paywalls/${encodeURIComponent(paywallId)}`,
        query: [],
        body: {
          name: input.name,
          description: input.description,
          policy: input.policy,
          routes: input.routes,
          metadata: input.metadata,
        },
        changedHint: true,
      };

    case "lightning_paywall_pause":
      return {
        method: "POST",
        path: `/api/lightning/paywalls/${encodeURIComponent(paywallId)}/pause`,
        query: [],
        body: {
          reason: input.reason,
        },
        changedHint: true,
      };

    case "lightning_paywall_resume":
      return {
        method: "POST",
        path: `/api/lightning/paywalls/${encodeURIComponent(paywallId)}/resume`,
        query: [],
        body: {
          reason: input.reason,
        },
        changedHint: true,
      };

    case "lightning_paywall_get":
      return {
        method: "GET",
        path: `/api/lightning/paywalls/${encodeURIComponent(paywallId)}`,
        query: [],
        body: undefined,
        changedHint: false,
      };

    case "lightning_paywall_list": {
      const query: Array<readonly [string, string]> = [];
      const status = asNonEmptyString(input.status);
      const limit = asNumber(input.limit);
      if (status) query.push(["status", status]);
      if (limit !== null) query.push(["limit", String(Math.max(1, Math.floor(limit)))]);
      return {
        method: "GET",
        path: "/api/lightning/paywalls",
        query,
        body: undefined,
        changedHint: false,
      };
    }

    case "lightning_paywall_settlement_list": {
      const query: Array<readonly [string, string]> = [];
      const limit = asNumber(input.limit);
      const beforeCreatedAtMs = asNumber(input.beforeCreatedAtMs);
      if (limit !== null) query.push(["limit", String(Math.max(1, Math.floor(limit)))]);
      if (beforeCreatedAtMs !== null) {
        query.push(["beforeCreatedAtMs", String(Math.floor(beforeCreatedAtMs))]);
      }

      if (paywallId) {
        return {
          method: "GET",
          path: `/api/lightning/paywalls/${encodeURIComponent(paywallId)}/settlements`,
          query,
          body: undefined,
          changedHint: false,
        };
      }

      return {
        method: "GET",
        path: "/api/lightning/settlements",
        query,
        body: undefined,
        changedHint: false,
      };
    }
  }
};

const withQuery = (url: URL, query: ReadonlyArray<readonly [string, string]>) => {
  for (const [key, value] of query) {
    url.searchParams.set(key, value);
  }
};

const sideEffect = (input: {
  readonly target: string;
  readonly method: string;
  readonly statusCode: number | null;
  readonly changed: boolean | null;
  readonly detail: string | null;
}): PaywallToolSideEffect => ({
  kind: "http_request",
  target: input.target,
  method: input.method,
  status_code: input.statusCode,
  changed: input.changed,
  detail: input.detail,
});

const changedFromResponse = (
  toolName: LightningPaywallToolName,
  body: unknown,
  fallback: boolean | null,
): boolean | null => {
  if (toolName === "lightning_paywall_get" || toolName === "lightning_paywall_list" || toolName === "lightning_paywall_settlement_list") {
    return false;
  }

  const changed = asBoolean(asRecord(body)?.changed);
  if (changed !== null) return changed;
  return fallback;
};

const finalizeOutput = async (input: {
  readonly startedAtMs: number;
  readonly paramsHash: string;
  readonly output: Omit<LightningPaywallToolOutput, "receipt">;
  readonly sideEffects: ReadonlyArray<PaywallToolSideEffect>;
}): Promise<LightningPaywallToolOutput> => {
  const outputBase: Omit<LightningPaywallToolOutput, "receipt"> = {
    status: input.output.status,
    denyCode: input.output.denyCode,
    denyReason: input.output.denyReason,
    errorCode: input.output.errorCode,
    errorMessage: input.output.errorMessage,
    httpStatus: input.output.httpStatus,
    requestId: input.output.requestId,
    paywall: input.output.paywall,
    paywalls: input.output.paywalls,
    settlements: input.output.settlements,
    nextCursor: input.output.nextCursor,
  };

  const outputHash = await sha256IdFromString(stableJson(outputBase));
  return {
    ...outputBase,
    receipt: {
      params_hash: input.paramsHash,
      output_hash: outputHash,
      latency_ms: Math.max(0, Date.now() - input.startedAtMs),
      side_effects: input.sideEffects,
    },
  };
};

const parseSuccess = (
  toolName: LightningPaywallToolName,
  body: unknown,
): Omit<LightningPaywallToolOutput, "receipt"> => {
  const rec = asRecord(body) ?? {};
  const requestId = asNonEmptyString(rec.requestId);

  if (
    toolName === "lightning_paywall_create" ||
    toolName === "lightning_paywall_update" ||
    toolName === "lightning_paywall_pause" ||
    toolName === "lightning_paywall_resume" ||
    toolName === "lightning_paywall_get"
  ) {
    return {
      ...defaultOutputState(),
      status: "ok",
      requestId,
      paywall: parsePaywallSummary(rec.paywall),
    };
  }

  if (toolName === "lightning_paywall_list") {
    const rowsRaw = Array.isArray(rec.paywalls) ? rec.paywalls : [];
    const paywalls = sortPaywalls(
      rowsRaw.map(parsePaywallSummary).filter((row): row is PaywallSummary => row !== null),
    );

    return {
      ...defaultOutputState(),
      status: "ok",
      requestId,
      paywalls,
    };
  }

  const rowsRaw = Array.isArray(rec.settlements) ? rec.settlements : [];
  const settlements = sortSettlements(
    rowsRaw
      .map(parseSettlementSummary)
      .filter((row): row is SettlementSummary => row !== null),
  );

  return {
    ...defaultOutputState(),
    status: "ok",
    requestId,
    settlements,
    nextCursor: asNumber(rec.nextCursor),
  };
};

const makeHeaders = (authToken: string | null, hasBody: boolean): Headers => {
  const headers = new Headers();
  headers.set("accept", "application/json");
  if (hasBody) headers.set("content-type", "application/json; charset=utf-8");
  if (authToken) headers.set("authorization", `Bearer ${authToken}`);
  return headers;
};

export async function executeLightningPaywallTool(
  options: ExecuteLightningPaywallToolOptions,
): Promise<LightningPaywallToolOutput> {
  const startedAtMs = Date.now();
  const paramsHash = await sha256IdFromString(stableJson(options.input));
  const cfg = controlPlaneConfig(options.env);
  const plan = buildRequestPlan(options.toolName, options.input);

  const fallbackTarget = cfg.baseUrl ? `${cfg.baseUrl}${plan.path}` : plan.path;

  if (!cfg.baseUrl) {
    return finalizeOutput({
      startedAtMs,
      paramsHash,
      output: {
        ...defaultOutputState(),
        status: "denied",
        denyCode: "not_configured",
        denyReason: "LIGHTNING_CONTROL_PLANE_BASE_URL is not configured",
      },
      sideEffects: [
        sideEffect({
          target: fallbackTarget,
          method: plan.method,
          statusCode: null,
          changed: plan.changedHint,
          detail: "control_plane_not_configured",
        }),
      ],
    });
  }

  const requestUrl = new URL(`${cfg.baseUrl}${plan.path}`);
  withQuery(requestUrl, plan.query);
  const fetchImpl = options.fetchImpl ?? fetch;

  const controller = new AbortController();
  const timeout = setTimeout(() => {
    controller.abort("timeout");
  }, cfg.timeoutMs);

  try {
    const requestInit: RequestInit = {
      method: plan.method,
      headers: makeHeaders(cfg.authToken, plan.body !== undefined),
      signal: controller.signal,
    };
    if (plan.body !== undefined) {
      requestInit.body = stableJson(plan.body);
    }

    const response = await fetchImpl(requestUrl.toString(), requestInit);

    const text = await response.text();
    const body = parseJson(text);
    const requestId =
      asNonEmptyString(asRecord(body)?.requestId) ??
      asNonEmptyString(response.headers.get("x-oa-request-id"));

    if (response.ok) {
      const parsed = parseSuccess(options.toolName, body);
      return finalizeOutput({
        startedAtMs,
        paramsHash,
        output: {
          ...parsed,
          httpStatus: response.status,
          requestId: parsed.requestId ?? requestId,
        },
        sideEffects: [
          sideEffect({
            target: requestUrl.toString(),
            method: plan.method,
            statusCode: response.status,
            changed: changedFromResponse(options.toolName, body, plan.changedHint),
            detail: null,
          }),
        ],
      });
    }

    const message = extractErrorMessage(body, response.status);
    if (isDeniedStatus(response.status)) {
      return finalizeOutput({
        startedAtMs,
        paramsHash,
        output: {
          ...defaultOutputState(),
          status: "denied",
          denyCode: classifyDenyCode(response.status, message),
          denyReason: message,
          httpStatus: response.status,
          requestId,
        },
        sideEffects: [
          sideEffect({
            target: requestUrl.toString(),
            method: plan.method,
            statusCode: response.status,
            changed: changedFromResponse(options.toolName, body, plan.changedHint),
            detail: message,
          }),
        ],
      });
    }

    return finalizeOutput({
      startedAtMs,
      paramsHash,
      output: {
        ...defaultOutputState(),
        status: "error",
        errorCode: `upstream_http_${response.status}`,
        errorMessage: message,
        httpStatus: response.status,
        requestId,
      },
      sideEffects: [
        sideEffect({
          target: requestUrl.toString(),
          method: plan.method,
          statusCode: response.status,
          changed: changedFromResponse(options.toolName, body, plan.changedHint),
          detail: message,
        }),
      ],
    });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    const lower = message.toLowerCase();
    const isTimeout = lower.includes("timeout") || lower.includes("abort");

    return finalizeOutput({
      startedAtMs,
      paramsHash,
      output: {
        ...defaultOutputState(),
        status: "error",
        errorCode: isTimeout ? "upstream_timeout" : "upstream_fetch_failed",
        errorMessage: message,
      },
      sideEffects: [
        sideEffect({
          target: requestUrl.toString(),
          method: plan.method,
          statusCode: null,
          changed: plan.changedHint,
          detail: message,
        }),
      ],
    });
  } finally {
    clearTimeout(timeout);
  }
}

export async function lightningPaywallToolInvalidInput(options: {
  readonly toolName: LightningPaywallToolName;
  readonly rawInput: unknown;
  readonly reason?: string;
}): Promise<LightningPaywallToolOutput> {
  const startedAtMs = Date.now();
  const paramsHash = await sha256IdFromString(stableJson(options.rawInput));
  const denyReason = options.reason?.trim() || "invalid_input";

  return finalizeOutput({
    startedAtMs,
    paramsHash,
    output: {
      ...defaultOutputState(),
      status: "denied",
      denyCode: "invalid_input",
      denyReason,
      errorCode: null,
      errorMessage: null,
      httpStatus: null,
      requestId: null,
    },
    sideEffects: [],
  });
}
