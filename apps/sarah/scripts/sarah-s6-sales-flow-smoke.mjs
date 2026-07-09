import { createJiti } from "jiti";

const allowPartial = process.env.SARAH_S6_ALLOW_PARTIAL === "1";
const liveWrites = process.env.SARAH_OPENAGENTS_LIVE_WRITES === "1";

function finish(payload, exitCode = 0) {
  console.log(JSON.stringify(payload, null, 2));
  process.exitCode = exitCode;
}

function blocked(reason, details = {}) {
  finish(
    {
      schema: "sarah.s6_sales_flow_smoke.v1",
      generatedAt: new Date().toISOString(),
      status: "blocked",
      reason,
      requiredEnv: [
        "SARAH_OPENAGENTS_LIVE_WRITES=1",
        "SARAH_OPENAGENTS_OPERATOR_TOKEN=<operator token for handoff>",
        "SARAH_OPENAGENTS_CHECKOUT_ENDPOINT=<OpenAgents checkout creation endpoint>",
      ],
      ...details,
    },
    allowPartial ? 0 : 2,
  );
}

if (!liveWrites) {
  blocked("live_writes_not_armed");
} else {
  const jiti = createJiti(import.meta.url);
  const { evaluateDealRules } = jiti("../src/lib/deal-rules.ts");
  const {
    captureOpenAgentsIntake,
    createOpenAgentsCheckoutLink,
    createOpenAgentsHumanHandoff,
  } = jiti("../src/lib/openagents-sales-client.ts");

  const runId = `s6-live-${Date.now()}`;
  const contactEmail = `${runId}@example.com`;
  const businessName = "Sarah S6 Live Smoke";
  const helpWith =
    "Testing Sarah's sales flow: qualification intake, configured quote trace, operator handoff, and checkout link creation.";

  try {
    const quote = evaluateDealRules({
      quoteKind: "credit_package",
      creditAmountUsdCents: 100_000,
      paymentMethod: "card",
      requestedTacticRef: null,
    });

    if (!quote.ok || quote.status !== "quoted") {
      throw new Error(`Deal rules did not return a quote: ${quote.message}`);
    }

    const intake = await captureOpenAgentsIntake({
      businessName,
      contactEmail,
      helpWith,
      phone: "+1 555 010 0606",
      requestSlackChannel: false,
      sourceRef: `sarah.s6_sales_flow.${runId}`,
      website: "https://example.com",
    });

    if (intake.mode !== "live" || typeof intake.intakeRef !== "string") {
      throw new Error(
        `Intake did not return a live row: ${JSON.stringify({
          mode: intake.mode,
          hasIntakeRef: Boolean(intake.intakeRef),
        })}`,
      );
    }

    let handoff = null;
    let handoffGate = null;
    try {
      handoff = await createOpenAgentsHumanHandoff({
        company: businessName,
        contactEmail,
        nextStep: "Operator should review Sarah S-6 live smoke handoff.",
        prospectName: "Sarah S6 Smoke",
        reason: "S-6 live smoke handoff proof",
        sourceRef: `sarah.s6_handoff.${runId}`,
        summary: helpWith,
        urgency: "normal",
      });
    } catch (error) {
      handoffGate = error instanceof Error ? error.message : String(error);
    }

    const checkout = await createOpenAgentsCheckoutLink({
      amountUsdCents: quote.totalUsdCents,
      businessName,
      contactEmail,
      dealRuleRefs: quote.ruleRefs,
      packageId: "quick_win_credit_pack",
      quoteRef: quote.quoteRef,
      signupId: intake.intakeRef,
      sourceRef: `sarah.s6_checkout.${runId}`,
      summary: helpWith,
    });

    const checkoutGate =
      checkout.mode === "live"
        ? null
        : "SARAH_OPENAGENTS_CHECKOUT_ENDPOINT is not configured, so checkout remains a dry-run quote.";
    const missingGates = [
      handoff?.mode === "live"
        ? null
        : (handoffGate ??
          "SARAH_OPENAGENTS_OPERATOR_TOKEN did not create a live handoff."),
      checkoutGate,
    ].filter(Boolean);

    finish(
      {
        schema: "sarah.s6_sales_flow_smoke.v1",
        generatedAt: new Date().toISOString(),
        status: missingGates.length === 0 ? "passed" : "partial_pass",
        baseUrl:
          process.env.SARAH_OPENAGENTS_BASE_URL?.replace(/\/+$/, "") ??
          "https://openagents.com",
        quote: {
          quoteRef: quote.quoteRef,
          totalUsdCents: quote.totalUsdCents,
          ruleRefs: quote.ruleRefs,
        },
        intake: {
          mode: intake.mode,
          intakeRef: intake.intakeRef,
          sourceRef: intake.sourceRef,
        },
        handoff:
          handoff === null
            ? { mode: null, error: handoffGate }
            : {
                mode: handoff.mode,
                handoffRef: handoff.handoffRef,
                sourceRef: handoff.sourceRef,
              },
        checkout: {
          mode: checkout.mode,
          checkoutRef: checkout.checkoutRef,
          checkoutUrl: checkout.checkoutUrl,
          moneyMovement: checkout.moneyMovement ?? null,
          openAgentsMode: checkout.openAgentsMode ?? null,
          amountUsdCents: checkout.amountUsdCents,
          quoteRef: checkout.quoteRef,
          dealRuleRefs: checkout.dealRuleRefs,
        },
        remainingExitGates: missingGates,
      },
      missingGates.length === 0 || allowPartial ? 0 : 2,
    );
  } catch (error) {
    blocked("sales_flow_smoke_failed", {
      error: error instanceof Error ? error.message : String(error),
    });
  }
}
