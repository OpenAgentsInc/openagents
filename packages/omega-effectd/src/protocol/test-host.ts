import type { OmegaEffectdHostRequest } from "./framed.ts";

export const makeOmegaEffectdTestHost = (
  onRequest?: (request: OmegaEffectdHostRequest) => void,
) => {
  let threadCounter = 0;
  return async (request: OmegaEffectdHostRequest): Promise<unknown> => {
    onRequest?.(request);
    const params = request.params as Record<string, unknown>;
    switch (request.method) {
      case "resolve_workspace":
        return { workspaceRef: params.expectedWorkspaceRef ?? "workspace.omega.supervised" };
      case "lane_readiness":
        return { known: true, admitted: true, fullAuto: true, state: "available" };
      case "create_thread":
        return { threadRef: `thread.omega.${++threadCounter}` };
      case "refresh_evidence":
        return { present: true, revision: 1, live: null, turns: [] };
      case "dispatch_turn":
        return { accepted: true };
      case "interrupt_turn":
        return { interrupted: true };
      case "append_system_note":
        return { appended: true };
    }
  };
};
