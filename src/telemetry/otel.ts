import { context, metrics, trace, SpanStatusCode, type Span } from "@opentelemetry/api";
import { OTLPMetricExporter } from "@opentelemetry/exporter-metrics-otlp-http";
import { OTLPTraceExporter } from "@opentelemetry/exporter-trace-otlp-http";
import { NodeSDK, type NodeSDKConfiguration } from "@opentelemetry/sdk-node";
import { PeriodicExportingMetricReader } from "@opentelemetry/sdk-metrics";
import { Resource } from "@opentelemetry/resources";
import { SemanticResourceAttributes } from "@opentelemetry/semantic-conventions";

const tracer = trace.getTracer("openagents");
const meter = metrics.getMeter("openagents");

const toolCalls = meter.createCounter("tool_calls", {
  description: "Counts tool calls executed by the agent",
});
const tokensUsed = meter.createCounter("tokens_used", {
  description: "Counts tokens consumed by the LLM (prompt/completion)",
});
const verificationRuns = meter.createCounter("verification_runs", {
  description: "Counts verification commands (typecheck/tests) and their outcomes",
});

let sdkStarted = false;
type SpanAttributes = Record<string, string | number | boolean | undefined>;

const buildExporterUrl = (envVar?: string, fallback?: string) => {
  if (envVar && envVar.length > 0) {
    return envVar;
  }
  if (!fallback) return undefined;
  const trimmed = fallback.endsWith("/") ? fallback.slice(0, -1) : fallback;
  return trimmed;
};

const startSdkIfEnabled = () => {
  if (sdkStarted) {
    return;
  }

  const endpoint = process.env.OTEL_EXPORTER_OTLP_ENDPOINT;
  const tracesEndpoint = buildExporterUrl(
    process.env.OTEL_EXPORTER_OTLP_TRACES_ENDPOINT,
    endpoint ? `${endpoint}/v1/traces` : undefined,
  );
  const metricsEndpoint = buildExporterUrl(
    process.env.OTEL_EXPORTER_OTLP_METRICS_ENDPOINT,
    endpoint ? `${endpoint}/v1/metrics` : undefined,
  );

  if (!tracesEndpoint && !metricsEndpoint) {
    sdkStarted = true; // mark checked to avoid repeated work in tight loops
    return;
  }

  const serviceName = process.env.OTEL_SERVICE_NAME ?? "openagents";
  const resource = new Resource({
    [SemanticResourceAttributes.SERVICE_NAME]: serviceName,
  });

  const sdkOptions: Partial<NodeSDKConfiguration> = {
    resource,
  };

  if (tracesEndpoint) {
    sdkOptions.traceExporter = new OTLPTraceExporter({ url: tracesEndpoint });
  }

  if (metricsEndpoint) {
    sdkOptions.metricReader = new PeriodicExportingMetricReader({
      exporter: new OTLPMetricExporter({ url: metricsEndpoint }),
    });
  }

  const sdk = new NodeSDK(sdkOptions);

  try {
    sdk.start();
  } catch {
    /* best-effort; ignore telemetry start failures */
  }

  sdkStarted = true;
};

export const startSpan = (name: string, attributes?: SpanAttributes): Span => {
  startSdkIfEnabled();
  return attributes ? tracer.startSpan(name, { attributes }) : tracer.startSpan(name);
};

export const endSpan = (span: Span | undefined, error?: unknown) => {
  if (!span) return;

  if (error) {
    if (error instanceof Error) {
      span.recordException(error);
      span.setStatus({ code: SpanStatusCode.ERROR, message: error.message });
    } else {
      span.setStatus({ code: SpanStatusCode.ERROR, message: String(error) });
    }
  } else {
    span.setStatus({ code: SpanStatusCode.OK });
  }
  span.end();
};

export const recordToolCall = (toolName: string, success: boolean) => {
  startSdkIfEnabled();
  toolCalls.add(1, {
    tool: toolName,
    outcome: success ? "success" : "error",
  });
};

export const recordTokenUsage = (params: {
  model?: string;
  promptTokens?: number | null | undefined;
  completionTokens?: number | null | undefined;
}) => {
  startSdkIfEnabled();
  const { model, promptTokens, completionTokens } = params;
  if (promptTokens && promptTokens > 0) {
    tokensUsed.add(promptTokens, { model: model ?? "unknown", kind: "prompt" });
  }
  if (completionTokens && completionTokens > 0) {
    tokensUsed.add(completionTokens, { model: model ?? "unknown", kind: "completion" });
  }
};

export const recordVerification = (kind: "typecheck" | "tests", success: boolean) => {
  startSdkIfEnabled();
  verificationRuns.add(1, {
    kind,
    outcome: success ? "success" : "failure",
  });
};

export const withSpanContext = <A>(span: Span, fn: () => A): A =>
  context.with(trace.setSpan(context.active(), span), fn);
