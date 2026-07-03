import { describe, expect, test } from "bun:test";
import type { Html } from "foldkit/html";

import "./setup";
import {
  businessAvailabilityBadge,
  businessIntakeForm,
  businessOfferingMenu,
  businessRateCard,
  defaultBusinessIntakeFieldNames,
  publicProofCaveat,
  quickWinLadder,
  type BusinessOffering,
  type BusinessRateCardPackage,
} from "../src";

type VNodeLike = Readonly<{
  sel?: string;
  text?: string;
  children?: ReadonlyArray<VNodeLike | string | null>;
  data?: {
    attrs?: Record<string, unknown>;
    props?: Record<string, unknown>;
    class?: Record<string, boolean>;
  };
}>;

const isVNodeLike = (value: unknown): value is VNodeLike =>
  typeof value === "object" && value !== null;

const attrsToString = (node: VNodeLike): string => {
  const attrs = node.data?.attrs ?? {};
  const props = node.data?.props ?? {};
  const classes = Object.entries(node.data?.class ?? {})
    .filter(([, enabled]) => enabled)
    .map(([className]) => className)
    .join(" ");
  const pairs = [
    ...Object.entries(attrs),
    ...Object.entries(props),
    ...(classes.length === 0 ? [] : [["class", classes] as const]),
  ];

  return pairs
    .filter(
      ([, value]) => value !== false && value !== undefined && value !== null,
    )
    .map(([name, value]) =>
      value === true ? ` ${name}` : ` ${name}="${String(value)}"`,
    )
    .join("");
};

const renderHtml = (html: Html): string => {
  if (html === null || !isVNodeLike(html)) {
    return "";
  }

  const tag = html.sel ?? "node";
  const children = (html.children ?? [])
    .map((child) =>
      typeof child === "string"
        ? child
        : child === null
          ? ""
          : renderHtml(child),
    )
    .join("");
  const text = html.text ?? "";

  return `<${tag}${attrsToString(html)}>${text}${children}</${tag}>`;
};

const offering: BusinessOffering = {
  title: "Coding & agent work",
  availability: "operator_assisted",
  what: "A coding agent works in your repo and returns evidence.",
  liveNow: "The coding runtime and negotiated labor loop are live.",
  caveat: "The priced intake-to-receipt product is operator-assisted today.",
  quickWin: "Quick win: fix one failing test with verification.",
  promiseIds: ["business.coding_quick_win.v1"],
};

const rateCardPackage: BusinessRateCardPackage = {
  title: "Quick Win",
  price: "$1,000-$5,000 fixed",
  scope: "One bounded deliverable in days.",
  receiptPlan: [
    "Confirmed intake scope",
    "Verification evidence",
    "Accepted-outcome receipt",
  ],
  caveat: "Operator-assisted today.",
  promiseIds: ["business.coding_quick_win.v1"],
};

describe("business landing components", () => {
  test("renders availability badges with stable state markers", () => {
    const rendered = renderHtml(
      businessAvailabilityBadge({
        availability: "available_now",
        mode: "light",
      }),
    );

    expect(rendered).toContain('data-ui-family="business/availability-badges"');
    expect(rendered).toContain('data-business-availability="available_now"');
    expect(rendered).toContain("Available now");
    expect(rendered).toContain("bg-public-landing-surface-muted");
    expect(rendered).toContain("text-public-landing-positive");
  });

  test("renders business offering menus and cards in dark and light modes", () => {
    const dark = renderHtml(
      businessOfferingMenu({
        offerings: [offering],
        mode: "dark",
      }),
    );
    const light = renderHtml(
      businessOfferingMenu({
        title: "Light menu",
        offerings: [offering],
        mode: "light",
      }),
    );

    expect(dark).toContain('data-ui-family="business/offering-menus"');
    expect(dark).toContain('data-ui-family="business/offering-cards"');
    expect(dark).toContain(
      'data-business-offering-title="Coding & agent work"',
    );
    expect(dark).toContain("Live now:");
    expect(dark).toContain("Current caveat:");
    expect(dark).toContain("business.coding_quick_win.v1");
    expect(light).toContain("Light menu");
    expect(light).toContain("bg-public-landing-surface");
    expect(light).toContain("text-public-landing-text");
  });

  test("renders the quick-win ladder and proof caveat family markers", () => {
    const ladder = renderHtml(
      quickWinLadder({
        steps: [
          {
            when: "Day 1",
            title: "Quick win",
            body: "Deliver one bounded task with evidence.",
          },
        ],
      }),
    );
    const caveat = renderHtml(
      publicProofCaveat({
        title: "Operator-assisted today",
        body: "This promise is yellow until a paid receipt lands.",
      }),
    );

    expect(ladder).toContain('data-ui-family="business/quick-win-ladders"');
    expect(ladder).toContain('data-business-ladder-step="Day 1"');
    expect(caveat).toContain('data-ui-family="business/proof-caveats"');
    expect(caveat).toContain("Operator-assisted today");
  });

  test("renders the rate card with package prices and receipt plans", () => {
    const rendered = renderHtml(
      businessRateCard({
        packages: [rateCardPackage],
      }),
    );

    expect(rendered).toContain('data-ui-family="business/rate-cards"');
    expect(rendered).toContain('data-ui-family="business/rate-card-packages"');
    expect(rendered).toContain('data-business-rate-card-package="Quick Win"');
    expect(rendered).toContain("$1,000-$5,000 fixed");
    expect(rendered).toContain("Receipt plan");
    expect(rendered).toContain("Accepted-outcome receipt");
    expect(rendered).toContain("Operator-assisted today.");
    expect(rendered).toContain("business.coding_quick_win.v1");
  });

  test("preserves the default public business intake field contract", () => {
    const rendered = renderHtml(
      businessIntakeForm({
        action: "/api/public/business-signup",
        pricingNote: "Paid runs are scoped with an explicit receipt plan.",
      }),
    );

    expect(rendered).toContain('data-ui-family="business/intake-forms"');
    expect(rendered).toContain('action="/api/public/business-signup"');
    expect(rendered).toContain(
      `name="${defaultBusinessIntakeFieldNames.businessName}"`,
    );
    expect(rendered).toContain(
      `name="${defaultBusinessIntakeFieldNames.contactEmail}"`,
    );
    expect(rendered).toContain(
      `name="${defaultBusinessIntakeFieldNames.website}"`,
    );
    expect(rendered).toContain(
      `name="${defaultBusinessIntakeFieldNames.phone}"`,
    );
    expect(rendered).toContain(
      `name="${defaultBusinessIntakeFieldNames.helpWith}"`,
    );
    expect(rendered).toContain(
      `name="${defaultBusinessIntakeFieldNames.requestSlackChannel}"`,
    );
    expect(rendered).toContain(
      `name="${defaultBusinessIntakeFieldNames.referralCode}"`,
    );
    expect(rendered).toContain('type="submit"');
    expect(rendered).toContain(
      "Paid runs are scoped with an explicit receipt plan.",
    );
  });
});
