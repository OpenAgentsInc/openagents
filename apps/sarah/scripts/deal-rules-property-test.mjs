import assert from "node:assert/strict";
import { createJiti } from "jiti";

const jiti = createJiti(import.meta.url);
const {
  DEFAULT_DEAL_RULES_CONFIG,
  evaluateDealRules,
  validateCheckoutQuoteTrace,
} = await jiti.import("../src/lib/deal-rules.ts");

const quote = input => {
  const result = evaluateDealRules(input);
  assert.equal(result.ok, true, JSON.stringify(result));
  return result;
};

const refusal = (input, reason) => {
  const result = evaluateDealRules(input);
  assert.equal(result.ok, false, JSON.stringify(result));
  assert.equal(result.reason, reason);
  return result;
};

const creditCases = [
  [100_000, "rule.credit_volume.usd_1000_2999.bonus_10pct", 110_000],
  [299_999, "rule.credit_volume.usd_1000_2999.bonus_10pct", 329_998],
  [300_000, "rule.credit_volume.usd_3000_4999.bonus_20pct", 360_000],
  [499_999, "rule.credit_volume.usd_3000_4999.bonus_20pct", 599_998],
  [500_000, "rule.credit_volume.usd_5000_plus.bonus_35pct", 675_000],
  [1_000_000, "rule.credit_volume.usd_5000_plus.bonus_35pct", 1_350_000],
];

for (const [amount, ruleRef, effectiveCreditUsdCents] of creditCases) {
  const result = quote({
    quoteKind: "credit_package",
    creditAmountUsdCents: amount,
    paymentMethod: "card",
  });
  assert.equal(result.totalUsdCents, amount);
  assert.equal(result.effectiveCreditUsdCents, effectiveCreditUsdCents);
  assert.ok(result.ruleRefs.includes(ruleRef));
  assert.ok(result.ruleRefs.includes("rule.cap.transaction_usd_10000"));
  assert.match(result.quoteRef, /^sarah_quote\.[a-f0-9]{24}$/);
}

for (const amount of [1, 99_999]) {
  refusal(
    {
      quoteKind: "credit_package",
      creditAmountUsdCents: amount,
      paymentMethod: "card",
    },
    "below_minimum_credit_package",
  );
}

for (const amount of [1_000_001, 2_000_000]) {
  const result = refusal(
    {
      quoteKind: "credit_package",
      creditAmountUsdCents: amount,
      paymentMethod: "card",
    },
    "above_transaction_cap",
  );
  assert.deepEqual(result.ruleRefs, ["rule.cap.transaction_usd_10000"]);
}

const bitcoin = quote({
  quoteKind: "credit_package",
  creditAmountUsdCents: 500_000,
  paymentMethod: "bitcoin",
});
assert.equal(bitcoin.totalUsdCents, 475_000);
assert.ok(bitcoin.ruleRefs.includes("rule.payment.bitcoin.discount_5pct"));

const unsignedModuleRefusal = refusal(
  {
    quoteKind: "module_bundle",
    moduleIds: [
      "module.internal_operations_ai",
      "module.customer_support_ai",
      "module.sales_employee_ai",
    ],
    paymentMethod: "card",
  },
  "module_pricing_owner_signoff_required",
);
assert.equal(unsignedModuleRefusal.ruleRefs.length, 0);

const signedFixtureConfig = {
  ...DEFAULT_DEAL_RULES_CONFIG,
  ownerSignature: {
    status: "signed",
    signedBy: "test-owner",
    signedAt: "2026-07-08T00:00:00.000Z",
  },
  modules: DEFAULT_DEAL_RULES_CONFIG.modules.map((module, index) => ({
    ...module,
    pricingStatus: "owner_signed",
    setupPriceUsdCents: [200_000, 300_000, 400_000][index],
  })),
};

const bundle = evaluateDealRules(
  {
    quoteKind: "module_bundle",
    moduleIds: [
      "module.internal_operations_ai",
      "module.customer_support_ai",
      "module.sales_employee_ai",
    ],
    paymentMethod: "card",
  },
  signedFixtureConfig,
);
assert.equal(bundle.ok, true, JSON.stringify(bundle));
assert.equal(bundle.listPriceUsdCents, 900_000);
assert.equal(bundle.totalUsdCents, 675_000);
assert.ok(
  bundle.ruleRefs.includes("rule.bundle.large_modules_3_plus.discount_25pct"),
);

const validTrace = validateCheckoutQuoteTrace({
  amountUsdCents: bitcoin.totalUsdCents,
  quoteRef: bitcoin.quoteRef,
  dealRuleRefs: bitcoin.ruleRefs,
});
assert.equal(validTrace.ok, true, JSON.stringify(validTrace));

const unknownTrace = validateCheckoutQuoteTrace({
  amountUsdCents: 100_000,
  quoteRef: "sarah_quote.000000000000000000000000",
  dealRuleRefs: ["rule.discount.make_it_up"],
});
assert.equal(unknownTrace.ok, false);
assert.equal(unknownTrace.reason, "unknown_rule_ref");

console.log(
  JSON.stringify(
    {
      ok: true,
      creditBoundaryCases: creditCases.length,
      bundleRuleRef:
        "rule.bundle.large_modules_3_plus.discount_25pct",
      checkoutTraceGuard: "unknown refs refused",
    },
    null,
    2,
  ),
);
