import { Effect } from "effect";

import {
  cleanupAuthedDotsGridBackground,
  hydrateAuthedDotsGridBackground,
  runAuthedShell,
} from "../../effuse-pages/authedShell";
import { dseCompileReportPageTemplate } from "../../effuse-pages/dseCompileReport";
import { dseEvalReportPageTemplate } from "../../effuse-pages/dseEvalReport";
import { dseOpsRunDetailPageTemplate } from "../../effuse-pages/dseOpsRunDetail";
import { dseOpsRunsPageTemplate } from "../../effuse-pages/dseOpsRuns";
import { dseSignaturePageTemplate } from "../../effuse-pages/dseSignature";
import {
  DseCompileReportPageDataAtom,
  DseEvalReportPageDataAtom,
  DseOpsRunDetailPageDataAtom,
  DseOpsRunsPageDataAtom,
  DseSignaturePageDataAtom,
  makeCompileReportKey,
  makeEvalReportKey,
  makeOpsRunDetailKey,
  makeSignatureKey,
} from "../../effect/atoms/dseViz";
import { SessionAtom } from "../../effect/atoms/session";

import type { Atom } from "@effect-atom/atom";
import type { Registry as AtomRegistry } from "@effect-atom/atom/Registry";

export type DseVizController = {
  readonly cleanup: () => void;
};

type DseRoute =
  | { readonly kind: "runs" }
  | { readonly kind: "run"; readonly runId: string }
  | { readonly kind: "signature"; readonly signatureId: string }
  | { readonly kind: "report"; readonly signatureId: string; readonly jobHash: string; readonly datasetHash: string }
  | { readonly kind: "evalReport"; readonly signatureId: string; readonly evalHash: string }
  | { readonly kind: "invalid"; readonly message: string };

const decodeSegment = (raw: string): string | null => {
  try {
    const s = decodeURIComponent(raw);
    return s && s.length > 0 ? s : null;
  } catch {
    return null;
  }
};

const parseDseRouteFromPathname = (pathname: string): DseRoute => {
  if (pathname === "/dse" || pathname === "/dse/") return { kind: "runs" };

  const opsPrefix = "/dse/ops/";
  if (pathname.startsWith(opsPrefix)) {
    const rest = pathname.slice(opsPrefix.length);
    if (!rest || rest.includes("/")) return { kind: "invalid", message: "Invalid ops run path." };
    const runId = decodeSegment(rest);
    if (!runId) return { kind: "invalid", message: "Invalid runId." };
    return { kind: "run", runId };
  }

  const sigPrefix = "/dse/signature/";
  if (pathname.startsWith(sigPrefix)) {
    const rest = pathname.slice(sigPrefix.length);
    if (!rest) return { kind: "invalid", message: "Invalid signature path." };
    const signatureId = decodeSegment(rest);
    if (!signatureId) return { kind: "invalid", message: "Invalid signatureId." };
    return { kind: "signature", signatureId };
  }

  const reportPrefix = "/dse/compile-report/";
  if (pathname.startsWith(reportPrefix)) {
    const rest = pathname.slice(reportPrefix.length);
    const parts = rest.split("/").filter((p) => p.length > 0);
    if (parts.length < 3) return { kind: "invalid", message: "Invalid compile report path." };
    const jobHash = decodeSegment(parts[0] ?? "");
    const datasetHash = decodeSegment(parts[1] ?? "");
    const signatureId = decodeSegment(parts.slice(2).join("/"));
    if (!signatureId || !jobHash || !datasetHash) return { kind: "invalid", message: "Invalid compile report params." };
    return { kind: "report", signatureId, jobHash, datasetHash };
  }

  const evalPrefix = "/dse/eval-report/";
  if (pathname.startsWith(evalPrefix)) {
    const rest = pathname.slice(evalPrefix.length);
    const parts = rest.split("/").filter((p) => p.length > 0);
    if (parts.length < 2) return { kind: "invalid", message: "Invalid eval report path." };
    const evalHash = decodeSegment(parts[0] ?? "");
    const signatureId = decodeSegment(parts.slice(1).join("/"));
    if (!signatureId || !evalHash) return { kind: "invalid", message: "Invalid eval report params." };
    return { kind: "evalReport", signatureId, evalHash };
  }

  return { kind: "invalid", message: "Unknown DSE route." };
};

export const mountDseVizController = (input: {
  readonly container: Element;
  readonly atoms: AtomRegistry;
}): DseVizController => {
  let unsubPage: (() => void) | null = null;

  const stopPage = () => {
    if (!unsubPage) return;
    unsubPage();
    unsubPage = null;
  };

  const route = parseDseRouteFromPathname(window.location.pathname);

  const renderAtom = <A>(atom: Atom.Atom<A>) => {
    stopPage();
    unsubPage = input.atoms.subscribe(
      atom,
      (data) => {
        const effect =
          route.kind === "runs"
            ? runAuthedShell(input.container, dseOpsRunsPageTemplate(data as any))
            : route.kind === "run"
              ? runAuthedShell(input.container, dseOpsRunDetailPageTemplate(data as any))
              : route.kind === "signature"
                ? runAuthedShell(input.container, dseSignaturePageTemplate(data as any))
                : route.kind === "report"
                  ? runAuthedShell(input.container, dseCompileReportPageTemplate(data as any))
                  : route.kind === "evalReport"
                    ? runAuthedShell(input.container, dseEvalReportPageTemplate(data as any))
                  : runAuthedShell(
                      input.container,
                      dseOpsRunsPageTemplate({ errorText: (route as any).message ?? "Invalid route", runs: [] }),
                    );

        Effect.runPromise(effect).catch(() => {});
      },
      { immediate: true },
    );
  };

  const startForUser = (userId: string) => {
    void userId;
    switch (route.kind) {
      case "runs":
        renderAtom(DseOpsRunsPageDataAtom(userId));
        return;
      case "run":
        renderAtom(DseOpsRunDetailPageDataAtom(makeOpsRunDetailKey(route.runId)));
        return;
      case "signature":
        renderAtom(DseSignaturePageDataAtom(makeSignatureKey(route.signatureId)));
        return;
      case "report":
        renderAtom(DseCompileReportPageDataAtom(makeCompileReportKey(route.signatureId, route.jobHash, route.datasetHash)));
        return;
      case "evalReport":
        renderAtom(DseEvalReportPageDataAtom(makeEvalReportKey(route.signatureId, route.evalHash)));
        return;
      default:
        stopPage();
        Effect.runPromise(
          runAuthedShell(
            input.container,
            dseOpsRunsPageTemplate({
              errorText: route.kind === "invalid" ? route.message : "Invalid route",
              runs: [],
            }),
          ),
        ).catch(() => {});
        return;
    }
  };

  const unsubSession = input.atoms.subscribe(
    SessionAtom,
    (session) => {
      if (session.userId) startForUser(session.userId);
    },
    { immediate: true },
  );

  Effect.runPromise(hydrateAuthedDotsGridBackground(input.container)).catch(() => {});

  return {
    cleanup: () => {
      unsubSession();
      stopPage();
      cleanupAuthedDotsGridBackground(input.container);
    },
  };
};
