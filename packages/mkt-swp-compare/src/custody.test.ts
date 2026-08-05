/**
 * Custody strip tests (issue #9318 §5): a custodial route can never read as
 * noncustodial, and the disclosure is never collapsed to one score.
 */
import { describe, expect, test } from "vite-plus/test";

import { custodyStrip } from "./custody.js";
import { custodialMintCustody, noncustodialScriptCustody } from "./testkit.js";
import { custodyStripView } from "./view.js";

describe("custodyStrip", () => {
  test("a verified script route reads noncustodial", () => {
    const strip = custodyStrip(noncustodialScriptCustody());
    expect(strip.stripClass).toBe("noncustodial");
    expect(strip.custodialControls).toEqual([]);
  });

  test("a mint/federation route is custodial and names who holds custody", () => {
    const strip = custodyStrip(custodialMintCustody());
    expect(strip.stripClass).toBe("custodial");
    const holders = strip.custodialControls.map(control => control.entry.holder);
    expect(holders).toContain("federation");
    expect(holders).toContain("mint");
    const dimensions = strip.custodialControls.map(control => control.dimension);
    expect(dimensions).toContain("funds_control");
    expect(dimensions).toContain("settlement_authority");
  });

  test("fail-closed: an unrecognised holder classifies custodial, never noncustodial", () => {
    const strip = custodyStrip(
      noncustodialScriptCustody({
        fundsControl: [{ leg: "source", phase: "funded", holder: "unknown" }],
      }),
    );
    expect(strip.stripClass).toBe("custodial");
  });

  test("a third party anywhere in execution control is custodial", () => {
    const strip = custodyStrip(
      noncustodialScriptCustody({
        executionControl: [
          { leg: "source", phase: "funded", holder: "third_party" },
        ],
      }),
    );
    expect(strip.stripClass).toBe("custodial");
  });

  test("recourse and reversibility rows cannot make a route read noncustodial", () => {
    // Custodial funds control stays custodial no matter how reassuring the
    // recourse/reversibility rows look.
    const strip = custodyStrip(
      custodialMintCustody({
        reversibility: [{ leg: "source", phase: "funded", holder: "contract" }],
        recourse: [{ leg: "source", phase: "exit", holder: "contract" }],
      }),
    );
    expect(strip.stripClass).toBe("custodial");
  });
});

describe("custodyStripView", () => {
  test("carries all six dimensions and both duration bounds, never one score", () => {
    const custody = noncustodialScriptCustody({
      maximumCustodyDurationSeconds: 172_800,
      maximumCustodyHeightBound: 288n,
    });
    const view = custodyStripView(custody);
    expect(view.disclosure.fundsControl.length).toBeGreaterThan(0);
    expect(view.disclosure.executionControl.length).toBeGreaterThan(0);
    expect(view.disclosure.settlementAuthority.length).toBeGreaterThan(0);
    expect(view.disclosure.reversibility.length).toBeGreaterThan(0);
    expect(view.disclosure.recourse.length).toBeGreaterThan(0);
    expect(view.disclosure.credentialExposure).toBe("none");
    // Both bounds present as distinct fields: the estimate is never
    // converted into consensus authority, and neither replaces the other.
    expect(view.maximumCustodyDurationSeconds).toBe(172_800);
    expect(view.maximumCustodyHeightBound).toBe(288n);
  });

  test("custodial and noncustodial strips render distinct labels", () => {
    const custodial = custodyStripView(custodialMintCustody());
    const noncustodial = custodyStripView(noncustodialScriptCustody());
    expect(custodial.label.key).not.toBe(noncustodial.label.key);
    expect(custodial.label.message).not.toBe(noncustodial.label.message);
  });
});
