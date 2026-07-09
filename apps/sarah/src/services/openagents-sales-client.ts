import { z } from "zod";
import { validateCheckoutQuoteTrace } from "./deal-rules";

const sourceRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,220}$/;

export const salesToolModeSchema = z.enum(["dry_run", "live"]);

export type SalesToolMode = z.infer<typeof salesToolModeSchema>;

export const intakeCaptureInputSchema = z.object({
  businessName: z.string().min(1).max(200),
  contactEmail: z.email().max(320),
  phone: z.string().min(1).max(80),
  helpWith: z.string().min(1).max(2_000),
  website: z.url().nullable().default(null),
  requestSlackChannel: z.boolean().default(false),
  sourceRef: z
    .string()
    .regex(sourceRefPattern)
    .default("sarah.voice_sales.v1"),
});

export const humanHandoffInputSchema = z.object({
  reason: z.string().min(1).max(400),
  summary: z.string().min(1).max(2_000),
  urgency: z.enum(["normal", "high"]).default("normal"),
  prospectName: z.string().min(1).max(160).nullable().default(null),
  contactEmail: z.email().max(320).nullable().default(null),
  company: z.string().min(1).max(200).nullable().default(null),
  nextStep: z.string().min(1).max(500).nullable().default(null),
  sourceRef: z
    .string()
    .regex(sourceRefPattern)
    .default("sarah.human_handoff.v1"),
});

export const checkoutLinkCreateInputSchema = z.object({
  packageId: z
    .enum([
      "quick_win_credit_pack",
      "fleet_sprint_credit_pack",
      "retainer_credit_pack",
    ])
    .default("quick_win_credit_pack"),
  amountUsdCents: z.number().int().min(100_000).max(1_000_000),
  businessName: z.string().min(1).max(200),
  contactEmail: z.email().max(320),
  quoteRef: z.string().regex(/^sarah_quote\.[a-f0-9]{24}$/),
  dealRuleRefs: z.array(z.string().min(1)).min(1),
  signupId: z.string().min(1).max(220).nullable().default(null),
  summary: z.string().min(1).max(2_000),
  sourceRef: z
    .string()
    .regex(sourceRefPattern)
    .default("sarah.checkout_link.v1"),
});

export type IntakeCaptureInput = z.infer<typeof intakeCaptureInputSchema>;
export type HumanHandoffInput = z.infer<typeof humanHandoffInputSchema>;
export type CheckoutLinkCreateInput = z.infer<
  typeof checkoutLinkCreateInputSchema
>;

function openAgentsBaseUrl() {
  return (
    process.env.SARAH_OPENAGENTS_BASE_URL?.replace(/\/+$/, "") ??
    "https://openagents.com"
  );
}

function liveWritesEnabled() {
  return process.env.SARAH_OPENAGENTS_LIVE_WRITES === "1";
}

function publicRef(prefix: string) {
  return `${prefix}.${crypto.randomUUID()}`;
}

function openAgentsBusinessSourceRef(sourceRef: string) {
  return /^(direct|unknown|ai_search|own_your_ai|apollo_model_custody|apollo_agent_readiness_[a-z0-9][a-z0-9_-]{0,63}|affiliate_[a-z0-9][a-z0-9_-]{0,63}|partner_[a-z0-9][a-z0-9_-]{0,63}|content_[a-z0-9][a-z0-9_-]{0,63}|vertical_[a-z0-9][a-z0-9_-]{0,63})$/.test(
    sourceRef,
  )
    ? sourceRef
    : "direct";
}

async function postJson(pathOrUrl: string, body: unknown, token?: string) {
  const url = pathOrUrl.startsWith("http")
    ? pathOrUrl
    : `${openAgentsBaseUrl()}${pathOrUrl}`;
  const headers: Record<string, string> = {
    accept: "application/json",
    "content-type": "application/json",
  };

  if (token) {
    headers.authorization = `Bearer ${token}`;
  }

  const response = await fetch(url, {
    method: "POST",
    headers,
    body: JSON.stringify(body),
  });
  const text = await response.text();
  const json = text ? JSON.parse(text) : null;

  if (!response.ok) {
    throw new Error(
      `OpenAgents API ${response.status}: ${
        typeof json?.reason === "string"
          ? json.reason
          : typeof json?.error === "string"
            ? json.error
            : text
      }`,
    );
  }

  return json;
}

export async function captureOpenAgentsIntake(input: IntakeCaptureInput) {
  if (!liveWritesEnabled()) {
    return {
      ok: true,
      mode: "dry_run" as SalesToolMode,
      intakeRef: publicRef("dry_run.business_signup"),
      sourceRef: input.sourceRef,
      submitted: input,
      message:
        "Captured the qualification intake in Sarah test mode; no production CRM row was written.",
    };
  }

  const json = await postJson("/api/public/business-signup", {
    ...input,
    sourceRef: openAgentsBusinessSourceRef(input.sourceRef),
    sourceRoute: "/sarah",
  });

  return {
    ok: true,
    mode: "live" as SalesToolMode,
    intakeRef: json?.record?.id ?? json?.request?.id ?? json?.id ?? null,
    sourceRef: input.sourceRef,
    response: json,
    message: "Created an OpenAgents business signup intake row.",
  };
}

export async function createOpenAgentsHumanHandoff(input: HumanHandoffInput) {
  const pipelineRef = publicRef("sarah.handoff");

  if (!liveWritesEnabled()) {
    return {
      ok: true,
      mode: "dry_run" as SalesToolMode,
      handoffRef: pipelineRef,
      sourceRef: input.sourceRef,
      submitted: input,
      message:
        "Prepared an operator handoff in Sarah test mode; no email or operator queue mutation was sent.",
    };
  }

  const token = process.env.SARAH_OPENAGENTS_OPERATOR_TOKEN;
  if (!token) {
    throw new Error(
      "SARAH_OPENAGENTS_OPERATOR_TOKEN is required for live handoff writes.",
    );
  }

  const json = await postJson(
    "/api/operator/business/pipeline",
    {
      pipelineRef,
      sourceRef: input.sourceRef,
      vertical: "ai_employee_sales",
      stage: "scope_scheduled",
      ownerRole: "operator",
      quotedBandLabel: input.urgency === "high" ? "handoff_high" : "handoff",
      quotedMinUsdCents: 0,
      quotedMaxUsdCents: 0,
      receiptRefs: [`sarah_handoff:${pipelineRef}`],
      blockerRef: null,
      nextActionDueAt: null,
      metadata: input,
    },
    token,
  );

  return {
    ok: true,
    mode: "live" as SalesToolMode,
    handoffRef: pipelineRef,
    sourceRef: input.sourceRef,
    response: json,
    message: "Created an OpenAgents operator handoff row.",
  };
}

export async function createOpenAgentsCheckoutLink(
  input: CheckoutLinkCreateInput,
) {
  const quoteTrace = validateCheckoutQuoteTrace({
    amountUsdCents: input.amountUsdCents,
    dealRuleRefs: input.dealRuleRefs,
    quoteRef: input.quoteRef,
  });
  if (!quoteTrace.ok) {
    return {
      ok: false,
      mode: "dry_run" as SalesToolMode,
      error: "checkout_quote_trace_required",
      quoteTrace,
      message:
        "Checkout was refused because the amount did not trace to configured Sarah deal-rule refs.",
    };
  }

  const checkoutRef = publicRef("sarah.checkout");
  const endpoint = process.env.SARAH_OPENAGENTS_CHECKOUT_ENDPOINT;

  if (!liveWritesEnabled() || !endpoint) {
    return {
      ok: true,
      mode: "dry_run" as SalesToolMode,
      checkoutRef,
      checkoutUrl: `${openAgentsBaseUrl()}/business`,
      amountUsdCents: input.amountUsdCents,
      dealRuleRefs: input.dealRuleRefs,
      packageId: input.packageId,
      quoteRef: input.quoteRef,
      sourceRef: input.sourceRef,
      message:
        "Prepared a test-mode checkout quote; no Stripe, Lightning, or credit ledger write occurred.",
    };
  }

  const token = process.env.SARAH_OPENAGENTS_OPERATOR_TOKEN;
  const buyerUserId = process.env.SARAH_OPENAGENTS_CHECKOUT_BUYER_USER_ID;
  const json = await postJson(
    endpoint,
    {
      ...input,
      ...(buyerUserId ? { buyerUserId } : {}),
    },
    token,
  );

  return {
    ok: true,
    mode: "live" as SalesToolMode,
    checkoutRef: json?.checkoutRef ?? checkoutRef,
    checkoutUrl: json?.checkoutUrl ?? json?.url ?? null,
    amountUsdCents: input.amountUsdCents,
    dealRuleRefs: input.dealRuleRefs,
    moneyMovement: json?.moneyMovement ?? null,
    openAgentsMode: json?.mode ?? null,
    packageId: input.packageId,
    quoteRef: input.quoteRef,
    sourceRef: input.sourceRef,
    response: json,
    message: "Created an OpenAgents checkout link.",
  };
}
