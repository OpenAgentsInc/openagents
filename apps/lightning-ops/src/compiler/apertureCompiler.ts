import { Effect, Layer } from "effect";

import type {
  CompiledApertureArtifact,
  CompiledApertureRule,
  CompileDiagnostic,
  ControlPlanePaywall,
  ControlPlanePaywallRoute,
} from "../contracts.js";
import { ApertureCompileValidationError } from "../errors.js";

import { configHashFromText, snapshotHashFromValue } from "./hash.js";
import { ApertureConfigCompilerService } from "./service.js";

type PathKind = "exact" | "prefix" | "regex" | "catchall";

type NormalizedCandidate = {
  readonly id: string;
  readonly paywallId: string;
  readonly ownerId: string;
  readonly routeId: string;
  readonly hostPattern: string;
  readonly pathPattern: string;
  readonly upstreamUrl: string;
  readonly protocol: "http" | "https";
  readonly timeoutMs: number;
  readonly priority: number;
  readonly amountMsats: number;
  readonly pathKind: PathKind;
  readonly hostSpecificity: number;
  readonly pathSpecificity: number;
};

const normalizeHostPattern = (input: string): string => input.trim().toLowerCase();

const normalizePathPattern = (input: string): string => {
  const trimmed = input.trim();
  if (trimmed.length === 0) return "/";
  return trimmed.startsWith("/") ? trimmed : `/${trimmed}`;
};

const classifyPath = (pathPattern: string): PathKind => {
  const value = normalizePathPattern(pathPattern);
  if (value === "/" || value === "/*") return "catchall";
  if (value.endsWith("/*")) return "prefix";
  if (/[\\^$+?()[\]{}|]/.test(value)) return "regex";
  if (value.includes("*")) return "regex";
  return "exact";
};

const hostSpecificity = (hostPattern: string): number => {
  if (hostPattern === "*") return 0;
  if (hostPattern.includes("*")) return 1;
  return 2;
};

const pathSpecificity = (pathPattern: string): number => {
  const kind = classifyPath(pathPattern);
  switch (kind) {
    case "exact":
      return 3;
    case "prefix":
      return 2;
    case "regex":
      return 1;
    case "catchall":
      return 0;
  }
};

const prefixFromPath = (pathPattern: string): string => {
  const normalized = normalizePathPattern(pathPattern);
  if (normalized.endsWith("/*")) return normalized.slice(0, -1);
  return normalized;
};

const pathOverlaps = (a: NormalizedCandidate, b: NormalizedCandidate): boolean => {
  if (a.pathPattern === b.pathPattern) return true;

  if (a.pathKind === "catchall" || b.pathKind === "catchall") return true;

  if (a.pathKind === "prefix") {
    const prefix = prefixFromPath(a.pathPattern);
    if (b.pathPattern.startsWith(prefix)) return true;
  }

  if (b.pathKind === "prefix") {
    const prefix = prefixFromPath(b.pathPattern);
    if (a.pathPattern.startsWith(prefix)) return true;
  }

  if (a.pathKind === "regex" || b.pathKind === "regex") return true;

  return false;
};

const moreGeneral = (a: NormalizedCandidate, b: NormalizedCandidate): boolean => {
  if (a.hostSpecificity < b.hostSpecificity) return true;
  if (a.hostSpecificity > b.hostSpecificity) return false;
  if (a.pathSpecificity < b.pathSpecificity) return true;
  if (a.pathSpecificity > b.pathSpecificity) return false;
  return a.pathPattern.length < b.pathPattern.length;
};

const compareCandidates = (a: NormalizedCandidate, b: NormalizedCandidate): number => {
  if (a.priority !== b.priority) return a.priority - b.priority;
  if (a.hostSpecificity !== b.hostSpecificity) return b.hostSpecificity - a.hostSpecificity;
  if (a.pathSpecificity !== b.pathSpecificity) return b.pathSpecificity - a.pathSpecificity;
  if (a.hostPattern !== b.hostPattern) return a.hostPattern.localeCompare(b.hostPattern);
  if (a.pathPattern !== b.pathPattern) return a.pathPattern.localeCompare(b.pathPattern);
  if (a.paywallId !== b.paywallId) return a.paywallId.localeCompare(b.paywallId);
  return a.routeId.localeCompare(b.routeId);
};

const makeInvalidInputDiagnostic = (
  paywallId: string,
  routeId: string | undefined,
  code: CompileDiagnostic["code"],
  message: string,
  details?: unknown,
): CompileDiagnostic => ({
  code,
  severity: "error",
  message,
  paywallId,
  routeId,
  details,
});

const validateAndNormalizeCandidates = (
  paywalls: ReadonlyArray<ControlPlanePaywall>,
): {
  readonly candidates: ReadonlyArray<NormalizedCandidate>;
  readonly diagnostics: ReadonlyArray<CompileDiagnostic>;
} => {
  const diagnostics: Array<CompileDiagnostic> = [];
  const candidates: Array<NormalizedCandidate> = [];

  for (const paywall of paywalls) {
    if (paywall.policy.pricingMode !== "fixed") {
      diagnostics.push(
        makeInvalidInputDiagnostic(
          paywall.paywallId,
          undefined,
          "invalid_pricing_mode",
          "Only fixed pricing mode is supported in Phase 2A",
          { pricingMode: paywall.policy.pricingMode },
        ),
      );
      continue;
    }

    if (!Number.isFinite(paywall.policy.fixedAmountMsats) || paywall.policy.fixedAmountMsats <= 0) {
      diagnostics.push(
        makeInvalidInputDiagnostic(
          paywall.paywallId,
          undefined,
          "missing_pricing",
          "fixedAmountMsats must be greater than zero",
          { fixedAmountMsats: paywall.policy.fixedAmountMsats },
        ),
      );
      continue;
    }

    for (const route of paywall.routes) {
      const hostPattern = normalizeHostPattern(route.hostPattern);
      const pathPattern = normalizePathPattern(route.pathPattern);
      const routeId = route.routeId;

      if (hostPattern.length === 0 || pathPattern.length === 0) {
        diagnostics.push(
          makeInvalidInputDiagnostic(
            paywall.paywallId,
            routeId,
            "invalid_route_pattern",
            "hostPattern and pathPattern must be non-empty",
            { hostPattern: route.hostPattern, pathPattern: route.pathPattern },
          ),
        );
        continue;
      }

      if (route.protocol !== "http" && route.protocol !== "https") {
        diagnostics.push(
          makeInvalidInputDiagnostic(
            paywall.paywallId,
            routeId,
            "missing_route_protocol",
            "route protocol must be http or https",
            { protocol: route.protocol },
          ),
        );
        continue;
      }

      const upstreamUrl = route.upstreamUrl.trim();
      if (upstreamUrl.length === 0) {
        diagnostics.push(
          makeInvalidInputDiagnostic(
            paywall.paywallId,
            routeId,
            "invalid_upstream_url",
            "upstreamUrl must be non-empty",
          ),
        );
        continue;
      }

      try {
        const parsed = new URL(upstreamUrl);
        const protocol = parsed.protocol === "http:" ? "http" : parsed.protocol === "https:" ? "https" : null;
        if (!protocol || protocol !== route.protocol) {
          diagnostics.push(
            makeInvalidInputDiagnostic(
              paywall.paywallId,
              routeId,
              "invalid_upstream_url",
              "upstreamUrl protocol must match route protocol",
              { upstreamUrl, routeProtocol: route.protocol },
            ),
          );
          continue;
        }
      } catch {
        diagnostics.push(
          makeInvalidInputDiagnostic(
            paywall.paywallId,
            routeId,
            "invalid_upstream_url",
            "upstreamUrl must be a valid URL",
            { upstreamUrl },
          ),
        );
        continue;
      }

      const timeoutMs = Number.isFinite(route.timeoutMs) && route.timeoutMs > 0 ? route.timeoutMs : 15_000;
      const priority = Number.isFinite(route.priority) ? route.priority : 0;
      const id = `${paywall.paywallId}:${route.routeId}`;

      candidates.push({
        id,
        paywallId: paywall.paywallId,
        ownerId: paywall.ownerId,
        routeId: route.routeId,
        hostPattern,
        pathPattern,
        upstreamUrl,
        protocol: route.protocol,
        timeoutMs,
        priority,
        amountMsats: paywall.policy.fixedAmountMsats,
        pathKind: classifyPath(pathPattern),
        hostSpecificity: hostSpecificity(hostPattern),
        pathSpecificity: pathSpecificity(pathPattern),
      });
    }
  }

  if (candidates.length === 0) {
    diagnostics.push({
      code: "no_compilable_routes",
      severity: "error",
      message: "No valid routes available to compile",
    });
  }

  return { candidates, diagnostics };
};

const validateSortedRoutes = (sorted: ReadonlyArray<NormalizedCandidate>): ReadonlyArray<CompileDiagnostic> => {
  const diagnostics: Array<CompileDiagnostic> = [];
  const routeKeySet = new Set<string>();

  for (const route of sorted) {
    const key = `${route.hostPattern}::${route.pathPattern}`;
    if (routeKeySet.has(key)) {
      diagnostics.push({
        code: "duplicate_route",
        severity: "error",
        message: `Duplicate host/path rule detected (${key})`,
        paywallId: route.paywallId,
        routeId: route.routeId,
      });
    }
    routeKeySet.add(key);
  }

  for (let i = 0; i < sorted.length; i += 1) {
    for (let j = i + 1; j < sorted.length; j += 1) {
      const primary = sorted[i]!;
      const secondary = sorted[j]!;

      if (primary.hostPattern !== secondary.hostPattern) continue;
      if (!pathOverlaps(primary, secondary)) continue;

      if (
        primary.priority === secondary.priority &&
        primary.pathSpecificity === secondary.pathSpecificity &&
        primary.pathPattern !== secondary.pathPattern
      ) {
        diagnostics.push({
          code: "ambiguous_route",
          severity: "error",
          message: `Ambiguous overlapping routes for host ${primary.hostPattern}`,
          paywallId: secondary.paywallId,
          routeId: secondary.routeId,
          relatedRouteId: primary.routeId,
          details: {
            primaryPath: primary.pathPattern,
            secondaryPath: secondary.pathPattern,
          },
        });
      }

      if (primary.priority <= secondary.priority && moreGeneral(primary, secondary)) {
        diagnostics.push({
          code: "first_match_shadowed",
          severity: "error",
          message: `Route ${secondary.routeId} is shadowed by earlier first-match rule ${primary.routeId}`,
          paywallId: secondary.paywallId,
          routeId: secondary.routeId,
          relatedRouteId: primary.routeId,
          details: {
            primaryPriority: primary.priority,
            secondaryPriority: secondary.priority,
            primaryPath: primary.pathPattern,
            secondaryPath: secondary.pathPattern,
          },
        });
      }
    }
  }

  return diagnostics;
};

const renderApertureYaml = (rules: ReadonlyArray<CompiledApertureRule>): string => {
  const lines: Array<string> = ["version: 1", "routes:"];
  for (const rule of rules) {
    lines.push(`  - id: ${rule.id}`);
    lines.push("    match:");
    lines.push(`      host: ${rule.hostPattern}`);
    lines.push(`      path: ${rule.pathPattern}`);
    lines.push("    upstream:");
    lines.push(`      url: ${rule.upstreamUrl}`);
    lines.push(`      protocol: ${rule.protocol}`);
    lines.push(`      timeout_ms: ${rule.timeoutMs}`);
    lines.push("    auth:");
    lines.push("      type: l402");
    lines.push(`      paywall_id: ${rule.paywallId}`);
    lines.push("    pricing:");
    lines.push("      mode: fixed_msats");
    lines.push(`      amount_msats: ${rule.amountMsats}`);
  }
  return `${lines.join("\n")}\n`;
};

const compileDeterministic = (
  paywalls: ReadonlyArray<ControlPlanePaywall>,
): Effect.Effect<CompiledApertureArtifact, ApertureCompileValidationError> =>
  Effect.gen(function* () {
    const normalized = validateAndNormalizeCandidates(paywalls);
    if (normalized.diagnostics.length > 0) {
      return yield* ApertureCompileValidationError.make({ diagnostics: normalized.diagnostics });
    }

    const sortedCandidates = [...normalized.candidates].sort(compareCandidates);
    const routeDiagnostics = validateSortedRoutes(sortedCandidates);
    if (routeDiagnostics.length > 0) {
      return yield* ApertureCompileValidationError.make({ diagnostics: routeDiagnostics });
    }

    const rules: Array<CompiledApertureRule> = sortedCandidates.map((candidate) => ({
      id: candidate.id,
      paywallId: candidate.paywallId,
      ownerId: candidate.ownerId,
      hostPattern: candidate.hostPattern,
      pathPattern: candidate.pathPattern,
      upstreamUrl: candidate.upstreamUrl,
      protocol: candidate.protocol,
      timeoutMs: candidate.timeoutMs,
      priority: candidate.priority,
      amountMsats: candidate.amountMsats,
    }));

    const apertureYaml = renderApertureYaml(rules);

    return {
      configHash: configHashFromText(apertureYaml),
      apertureYaml,
      rules,
      diagnostics: [],
      ruleCount: rules.length,
      valid: true,
    };
  });

export const snapshotHash = (paywalls: ReadonlyArray<ControlPlanePaywall>): string =>
  snapshotHashFromValue(
    paywalls.map((paywall) => ({
      paywallId: paywall.paywallId,
      ownerId: paywall.ownerId,
      status: paywall.status,
      policy: {
        pricingMode: paywall.policy.pricingMode,
        fixedAmountMsats: paywall.policy.fixedAmountMsats,
        maxPerRequestMsats: paywall.policy.maxPerRequestMsats,
        killSwitch: paywall.policy.killSwitch,
      },
      routes: paywall.routes.map((route: ControlPlanePaywallRoute) => ({
        routeId: route.routeId,
        hostPattern: normalizeHostPattern(route.hostPattern),
        pathPattern: normalizePathPattern(route.pathPattern),
        upstreamUrl: route.upstreamUrl,
        protocol: route.protocol,
        timeoutMs: route.timeoutMs,
        priority: route.priority,
      })),
    })),
  );

export const ApertureConfigCompilerLive = Layer.succeed(
  ApertureConfigCompilerService,
  ApertureConfigCompilerService.of({
    compile: compileDeterministic,
    snapshotHash,
  }),
);
