import webKnobs from "./detail-knobs.json" with { type: "json" };
import {
  budgetForProfile,
  cssVariables,
  DetailController,
  knobValues,
  REPORT_SCHEMA,
  validateKnobs,
  type Budget,
  type ControllerConfig,
  type DetailReport,
  type DeviceProfile,
  type KnobDecl,
  type KnobValue,
  type Sample,
} from "./detail.ts";
import {
  rendererDetailFromKnobs,
  type RendererDetail,
} from "../renderer/detail-settings.ts";

/** The web renderer's knob declarations; a test keeps them equal to the crate's. */
export const WEB_KNOBS: readonly KnobDecl[] = Object.freeze(
  validateKnobs(webKnobs as KnobDecl[]),
);

/** Where interface knobs are written: an element's style. */
export interface StyleTarget {
  style: { setProperty(name: string, value: string): void };
  dataset?: Record<string, string | undefined>;
}

/**
 * One session's profile, budget, controller, and knobs. The world session
 * feeds it frames and applies `renderer()` when Detail changes; the shell
 * applies `applyInterface()` to the document.
 */
export class DetailDriver {
  readonly knobs: readonly KnobDecl[];
  readonly budget: Budget;
  readonly controller: DetailController;
  private values: KnobValue[];

  constructor(
    public profile: DeviceProfile,
    options: {
      knobs?: readonly KnobDecl[];
      config?: Partial<ControllerConfig>;
    } = {},
  ) {
    this.knobs = validateKnobs(options.knobs ?? WEB_KNOBS);
    this.budget = budgetForProfile(profile);
    this.controller = new DetailController({
      budget_ms: this.budget.frame_ms,
      gpu_budget_ms: this.budget.gpu_ms,
      ...options.config,
    });
    this.values = this.compute();
  }

  private compute() {
    return knobValues(this.knobs, this.controller.detail(), this.profile.prefs);
  }
  get detail() {
    return this.controller.detail();
  }
  /** Feed one frame; true when Detail changed. */
  observe(sample: Sample): boolean {
    if (this.controller.observe(sample) === undefined) return false;
    this.values = this.compute();
    return true;
  }
  /** Pin Detail (captures, benchmarks); returns whether settings changed. */
  hold(value = this.detail): boolean {
    const before = this.detail;
    this.controller.hold(value);
    this.values = this.compute();
    return this.detail !== before;
  }
  release(): boolean {
    const before = this.detail;
    this.controller.release();
    this.values = this.compute();
    return this.detail !== before;
  }
  /** Preferences or the viewport changed; Detail is kept. */
  updateProfile(profile: DeviceProfile) {
    this.profile = profile;
    this.values = this.compute();
  }
  knobValues(): readonly KnobValue[] {
    return this.values;
  }
  renderer(): RendererDetail {
    return rendererDetailFromKnobs(this.values);
  }
  cssVariables() {
    return cssVariables(this.detail, this.values);
  }
  /**
   * Write `--detail*` on `target` (usually `document.documentElement`), and
   * `data-detail-motion="off"` when motion is at zero, for styles that
   * can't multiply.
   */
  applyInterface(target: StyleTarget) {
    for (const [name, value] of this.cssVariables())
      target.style.setProperty(name, value);
    if (target.dataset) {
      const motion = this.values.find((v) => v.knob === "motion");
      target.dataset.detailMotion = motion?.value === 0 ? "off" : "on";
    }
  }
  report(): DetailReport {
    return {
      schema: REPORT_SCHEMA,
      profile: structuredClone(this.profile),
      budget: { ...this.budget },
      controller: this.controller.state(),
      knobs: this.values.map((v) => ({ ...v })),
    };
  }
}
