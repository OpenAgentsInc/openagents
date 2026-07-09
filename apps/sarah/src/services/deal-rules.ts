import { createHash } from "node:crypto";
import { z } from "zod";

const basisPoints = z.number().int().min(0).max(10_000);
const cents = z.number().int().min(0);

export const dealModuleSchema = z.object({
  id: z.string().min(1),
  name: z.string().min(1),
  size: z.enum(["small", "medium", "large"]),
  serviceLadderRef: z.string().min(1),
  promiseStateRef: z.string().min(1),
  setupPriceUsdCents: cents.nullable(),
  pricingStatus: z.enum(["owner_signed", "owner_pricing_required"]),
});

export const dealRulesConfigSchema = z.object({
  schema: z.literal("sarah.deal_rules.v1"),
  version: z.string().min(1),
  ownerSignature: z.object({
    status: z.enum(["pending_owner_signoff", "signed"]),
    signedBy: z.string().nullable(),
    signedAt: z.string().nullable(),
  }),
  transactionCapUsdCents: cents,
  creditVolumeTiers: z.array(
    z.object({
      ruleRef: z.string().min(1),
      minUsdCents: cents,
      maxUsdCents: cents.nullable(),
      bonusBasisPoints: basisPoints,
    }),
  ),
  bitcoinDiscount: z.object({
    ruleRef: z.string().min(1),
    discountBasisPoints: basisPoints,
  }),
  bundleRules: z.array(
    z.object({
      ruleRef: z.string().min(1),
      moduleSize: z.enum(["small", "medium", "large"]),
      minModules: z.number().int().min(1),
      discountBasisPoints: basisPoints,
    }),
  ),
  tactics: z.array(
    z.object({
      tacticRef: z.string().min(1),
      status: z.enum(["parked_owner_action", "armed"]),
    }),
  ),
  modules: z.array(dealModuleSchema),
});

export type DealRulesConfig = z.infer<typeof dealRulesConfigSchema>;

export const DEFAULT_DEAL_RULES_CONFIG: DealRulesConfig = {
  schema: "sarah.deal_rules.v1",
  version: "sarah.deal_rules.v1.2026-07-08",
  ownerSignature: {
    status: "pending_owner_signoff",
    signedBy: null,
    signedAt: null,
  },
  transactionCapUsdCents: 1_000_000,
  creditVolumeTiers: [
    {
      ruleRef: "rule.credit_volume.usd_1000_2999.bonus_10pct",
      minUsdCents: 100_000,
      maxUsdCents: 299_999,
      bonusBasisPoints: 1_000,
    },
    {
      ruleRef: "rule.credit_volume.usd_3000_4999.bonus_20pct",
      minUsdCents: 300_000,
      maxUsdCents: 499_999,
      bonusBasisPoints: 2_000,
    },
    {
      ruleRef: "rule.credit_volume.usd_5000_plus.bonus_35pct",
      minUsdCents: 500_000,
      maxUsdCents: null,
      bonusBasisPoints: 3_500,
    },
  ],
  bitcoinDiscount: {
    ruleRef: "rule.payment.bitcoin.discount_5pct",
    discountBasisPoints: 500,
  },
  bundleRules: [
    {
      ruleRef: "rule.bundle.large_modules_3_plus.discount_25pct",
      moduleSize: "large",
      minModules: 3,
      discountBasisPoints: 2_500,
    },
  ],
  tactics: [
    {
      tacticRef: "tactic.close_on_call",
      status: "parked_owner_action",
    },
  ],
  modules: [
    {
      id: "module.internal_operations_ai",
      name: "Internal Operations AI module",
      size: "large",
      serviceLadderRef: "service_ladder.ai_employee.large_module",
      promiseStateRef: "promise_registry.required",
      setupPriceUsdCents: null,
      pricingStatus: "owner_pricing_required",
    },
    {
      id: "module.customer_support_ai",
      name: "Customer Support AI module",
      size: "large",
      serviceLadderRef: "service_ladder.ai_employee.large_module",
      promiseStateRef: "promise_registry.required",
      setupPriceUsdCents: null,
      pricingStatus: "owner_pricing_required",
    },
    {
      id: "module.sales_employee_ai",
      name: "Sales Employee AI module",
      size: "large",
      serviceLadderRef: "service_ladder.ai_employee.large_module",
      promiseStateRef: "promise_registry.required",
      setupPriceUsdCents: null,
      pricingStatus: "owner_pricing_required",
    },
  ],
};

export const dealRulesEvaluateInputSchema = z
  .object({
    quoteKind: z.enum(["credit_package", "module_bundle"]),
    creditAmountUsdCents: z.number().int().min(1).max(2_000_000).optional(),
    moduleIds: z.array(z.string().min(1)).min(1).max(20).optional(),
    paymentMethod: z.enum(["card", "bitcoin"]).default("card"),
    requestedTacticRef: z.string().min(1).nullable().default(null),
  })
  .superRefine((input, ctx) => {
    if (
      input.quoteKind === "credit_package" &&
      input.creditAmountUsdCents == null
    ) {
      ctx.addIssue({
        code: "custom",
        message: "creditAmountUsdCents is required for credit package quotes.",
        path: ["creditAmountUsdCents"],
      });
    }

    if (input.quoteKind === "module_bundle" && !input.moduleIds?.length) {
      ctx.addIssue({
        code: "custom",
        message: "moduleIds are required for module bundle quotes.",
        path: ["moduleIds"],
      });
    }
  });

type DealRulesEvaluateRawInput = z.infer<typeof dealRulesEvaluateInputSchema>;

export type DealRulesEvaluateInput =
  | {
      quoteKind: "credit_package";
      creditAmountUsdCents: number;
      paymentMethod: "card" | "bitcoin";
      requestedTacticRef: string | null;
    }
  | {
      quoteKind: "module_bundle";
      moduleIds: string[];
      paymentMethod: "card" | "bitcoin";
      requestedTacticRef: string | null;
    };

export type AppliedDealRule = {
  ruleRef: string;
  label: string;
  amountUsdCentsDelta: number;
};

export type DealRulesEvaluateOutput =
  | {
      ok: true;
      status: "quoted";
      schema: "sarah.deal_rules.v1";
      configVersion: string;
      quoteRef: string;
      quoteKind: DealRulesEvaluateInput["quoteKind"];
      listPriceUsdCents: number;
      totalUsdCents: number;
      effectiveCreditUsdCents: number | null;
      appliedRules: AppliedDealRule[];
      ruleRefs: string[];
      message: string;
    }
  | {
      ok: false;
      status: "escalate";
      schema: "sarah.deal_rules.v1";
      configVersion: string;
      reason:
        | "above_transaction_cap"
        | "below_minimum_credit_package"
        | "module_pricing_owner_signoff_required"
        | "unknown_module"
        | "tactic_not_armed";
      missingModuleIds?: string[];
      ruleRefs: string[];
      message: string;
    };

function centsDelta(amountUsdCents: number, basisPointsValue: number) {
  return Math.floor((amountUsdCents * basisPointsValue) / 10_000);
}

function quoteRefFor(payload: unknown) {
  const digest = createHash("sha256")
    .update(JSON.stringify(payload))
    .digest("hex")
    .slice(0, 24);
  return `sarah_quote.${digest}`;
}

function capRuleRef(config: DealRulesConfig) {
  return `rule.cap.transaction_usd_${Math.floor(
    config.transactionCapUsdCents / 100,
  )}`;
}

export function knownDealRuleRefs(config = DEFAULT_DEAL_RULES_CONFIG) {
  return new Set([
    capRuleRef(config),
    ...config.creditVolumeTiers.map(tier => tier.ruleRef),
    config.bitcoinDiscount.ruleRef,
    ...config.bundleRules.map(rule => rule.ruleRef),
    ...config.tactics.map(tactic => tactic.tacticRef),
  ]);
}

function tacticEscalation(
  input: DealRulesEvaluateInput,
  config: DealRulesConfig,
): DealRulesEvaluateOutput | null {
  if (!input.requestedTacticRef) return null;

  const tactic = config.tactics.find(
    candidate => candidate.tacticRef === input.requestedTacticRef,
  );
  if (!tactic || tactic.status !== "armed") {
    return {
      ok: false,
      status: "escalate",
      schema: "sarah.deal_rules.v1",
      configVersion: config.version,
      reason: "tactic_not_armed",
      ruleRefs: [],
      message:
        "That tactic is not armed by the owner, so Sarah cannot apply it or describe it as available.",
    };
  }

  return null;
}

export function evaluateDealRules(
  rawInput: DealRulesEvaluateRawInput,
  rawConfig: DealRulesConfig = DEFAULT_DEAL_RULES_CONFIG,
): DealRulesEvaluateOutput {
  const input = normalizeDealRulesEvaluateInput(rawInput);
  const config = dealRulesConfigSchema.parse(rawConfig);
  const tacticBlocked = tacticEscalation(input, config);
  if (tacticBlocked) return tacticBlocked;

  if (input.quoteKind === "credit_package") {
    return evaluateCreditPackage(input, config);
  }

  return evaluateModuleBundle(input, config);
}

function normalizeDealRulesEvaluateInput(
  rawInput: DealRulesEvaluateRawInput,
): DealRulesEvaluateInput {
  const input = dealRulesEvaluateInputSchema.parse(rawInput);

  if (input.quoteKind === "credit_package") {
    return {
      quoteKind: input.quoteKind,
      creditAmountUsdCents: input.creditAmountUsdCents!,
      paymentMethod: input.paymentMethod,
      requestedTacticRef: input.requestedTacticRef,
    };
  }

  return {
    quoteKind: input.quoteKind,
    moduleIds: input.moduleIds!,
    paymentMethod: input.paymentMethod,
    requestedTacticRef: input.requestedTacticRef,
  };
}

function evaluateCreditPackage(
  input: Extract<DealRulesEvaluateInput, { quoteKind: "credit_package" }>,
  config: DealRulesConfig,
): DealRulesEvaluateOutput {
  if (input.creditAmountUsdCents < 100_000) {
    return {
      ok: false,
      status: "escalate",
      schema: "sarah.deal_rules.v1",
      configVersion: config.version,
      reason: "below_minimum_credit_package",
      ruleRefs: [],
      message:
        "The requested credit package is below Sarah's configured minimum; escalate instead of quoting.",
    };
  }

  if (input.creditAmountUsdCents > config.transactionCapUsdCents) {
    return {
      ok: false,
      status: "escalate",
      schema: "sarah.deal_rules.v1",
      configVersion: config.version,
      reason: "above_transaction_cap",
      ruleRefs: [capRuleRef(config)],
      message:
        "The requested amount is above Sarah's per-transaction cap; prepare a human handoff.",
    };
  }

  const tier = config.creditVolumeTiers.find(
    candidate =>
      input.creditAmountUsdCents >= candidate.minUsdCents &&
      (candidate.maxUsdCents === null ||
        input.creditAmountUsdCents <= candidate.maxUsdCents),
  );
  if (!tier) {
    return {
      ok: false,
      status: "escalate",
      schema: "sarah.deal_rules.v1",
      configVersion: config.version,
      reason: "below_minimum_credit_package",
      ruleRefs: [],
      message:
        "No credit-volume tier matches this amount; Sarah cannot quote it.",
    };
  }

  const bonus = centsDelta(
    input.creditAmountUsdCents,
    tier.bonusBasisPoints,
  );
  const appliedRules: AppliedDealRule[] = [
    {
      ruleRef: capRuleRef(config),
      label: "Per-transaction cap",
      amountUsdCentsDelta: 0,
    },
    {
      ruleRef: tier.ruleRef,
      label: `${tier.bonusBasisPoints / 100}% credit-volume bonus`,
      amountUsdCentsDelta: bonus,
    },
  ];
  let totalUsdCents = input.creditAmountUsdCents;

  if (input.paymentMethod === "bitcoin") {
    const discount = centsDelta(
      totalUsdCents,
      config.bitcoinDiscount.discountBasisPoints,
    );
    totalUsdCents -= discount;
    appliedRules.push({
      ruleRef: config.bitcoinDiscount.ruleRef,
      label: `${config.bitcoinDiscount.discountBasisPoints / 100}% Bitcoin discount`,
      amountUsdCentsDelta: -discount,
    });
  }

  const ruleRefs = appliedRules.map(rule => rule.ruleRef);
  const quotePayload = {
    configVersion: config.version,
    quoteKind: input.quoteKind,
    listPriceUsdCents: input.creditAmountUsdCents,
    totalUsdCents,
    effectiveCreditUsdCents: input.creditAmountUsdCents + bonus,
    ruleRefs,
  };

  return {
    ok: true,
    status: "quoted",
    schema: "sarah.deal_rules.v1",
    configVersion: config.version,
    quoteRef: quoteRefFor(quotePayload),
    quoteKind: input.quoteKind,
    listPriceUsdCents: input.creditAmountUsdCents,
    totalUsdCents,
    effectiveCreditUsdCents: input.creditAmountUsdCents + bonus,
    appliedRules,
    ruleRefs,
    message:
      "Quoted a credit package from configured credit-volume and payment rules only.",
  };
}

function evaluateModuleBundle(
  input: Extract<DealRulesEvaluateInput, { quoteKind: "module_bundle" }>,
  config: DealRulesConfig,
): DealRulesEvaluateOutput {
  const modulesById = new Map(config.modules.map(module => [module.id, module]));
  const selectedModules = input.moduleIds.map(id => modulesById.get(id));
  const missingModuleIds = input.moduleIds.filter(id => !modulesById.has(id));
  if (missingModuleIds.length > 0) {
    return {
      ok: false,
      status: "escalate",
      schema: "sarah.deal_rules.v1",
      configVersion: config.version,
      reason: "unknown_module",
      missingModuleIds,
      ruleRefs: [],
      message:
        "At least one requested module is not in the owner-authored catalog; escalate instead of quoting.",
    };
  }

  const unsignedModules = selectedModules.filter(
    module =>
      module?.pricingStatus !== "owner_signed" ||
      module.setupPriceUsdCents === null,
  );
  if (config.ownerSignature.status !== "signed" || unsignedModules.length > 0) {
    return {
      ok: false,
      status: "escalate",
      schema: "sarah.deal_rules.v1",
      configVersion: config.version,
      reason: "module_pricing_owner_signoff_required",
      missingModuleIds: unsignedModules.map(module => module?.id ?? "unknown"),
      ruleRefs: [],
      message:
        "Module pricing is not owner-signed, so Sarah must get a firm number from a human.",
    };
  }

  const pricedModules = selectedModules.map(module => module!);
  const listPriceUsdCents = pricedModules.reduce(
    (sum, module) => sum + (module.setupPriceUsdCents ?? 0),
    0,
  );
  if (listPriceUsdCents > config.transactionCapUsdCents) {
    return {
      ok: false,
      status: "escalate",
      schema: "sarah.deal_rules.v1",
      configVersion: config.version,
      reason: "above_transaction_cap",
      ruleRefs: [capRuleRef(config)],
      message:
        "The configured module bundle is above Sarah's per-transaction cap; prepare a human handoff.",
    };
  }

  const appliedRules: AppliedDealRule[] = [
    {
      ruleRef: capRuleRef(config),
      label: "Per-transaction cap",
      amountUsdCentsDelta: 0,
    },
  ];
  let totalUsdCents = listPriceUsdCents;

  for (const rule of config.bundleRules) {
    const matchingModules = pricedModules.filter(
      module => module.size === rule.moduleSize,
    );
    if (matchingModules.length >= rule.minModules) {
      const discountBase = matchingModules.reduce(
        (sum, module) => sum + (module.setupPriceUsdCents ?? 0),
        0,
      );
      const discount = centsDelta(discountBase, rule.discountBasisPoints);
      totalUsdCents -= discount;
      appliedRules.push({
        ruleRef: rule.ruleRef,
        label: `${rule.discountBasisPoints / 100}% ${rule.moduleSize}-module bundle discount`,
        amountUsdCentsDelta: -discount,
      });
    }
  }

  if (input.paymentMethod === "bitcoin") {
    const discount = centsDelta(
      totalUsdCents,
      config.bitcoinDiscount.discountBasisPoints,
    );
    totalUsdCents -= discount;
    appliedRules.push({
      ruleRef: config.bitcoinDiscount.ruleRef,
      label: `${config.bitcoinDiscount.discountBasisPoints / 100}% Bitcoin discount`,
      amountUsdCentsDelta: -discount,
    });
  }

  const ruleRefs = appliedRules.map(rule => rule.ruleRef);
  const quotePayload = {
    configVersion: config.version,
    quoteKind: input.quoteKind,
    listPriceUsdCents,
    moduleIds: input.moduleIds,
    totalUsdCents,
    ruleRefs,
  };

  return {
    ok: true,
    status: "quoted",
    schema: "sarah.deal_rules.v1",
    configVersion: config.version,
    quoteRef: quoteRefFor(quotePayload),
    quoteKind: input.quoteKind,
    listPriceUsdCents,
    totalUsdCents,
    effectiveCreditUsdCents: null,
    appliedRules,
    ruleRefs,
    message:
      "Quoted an owner-signed module bundle from configured module prices and bundle rules only.",
  };
}

export const checkoutQuoteTraceSchema = z.object({
  quoteRef: z.string().regex(/^sarah_quote\.[a-f0-9]{24}$/),
  dealRuleRefs: z.array(z.string().min(1)).min(1),
  amountUsdCents: z.number().int().min(1),
});

export function validateCheckoutQuoteTrace(
  input: z.infer<typeof checkoutQuoteTraceSchema>,
  config: DealRulesConfig = DEFAULT_DEAL_RULES_CONFIG,
) {
  const parsed = checkoutQuoteTraceSchema.safeParse(input);
  if (!parsed.success) {
    return {
      ok: false as const,
      reason: "invalid_quote_trace",
      issues: parsed.error.issues,
    };
  }

  if (parsed.data.amountUsdCents > config.transactionCapUsdCents) {
    return {
      ok: false as const,
      reason: "above_transaction_cap",
      ruleRefs: [capRuleRef(config)],
    };
  }

  const knownRefs = knownDealRuleRefs(config);
  const unknownRuleRefs = parsed.data.dealRuleRefs.filter(
    ref => !knownRefs.has(ref),
  );
  if (unknownRuleRefs.length > 0) {
    return {
      ok: false as const,
      reason: "unknown_rule_ref",
      unknownRuleRefs,
    };
  }

  return {
    ok: true as const,
    quoteRef: parsed.data.quoteRef,
    ruleRefs: parsed.data.dealRuleRefs,
  };
}
