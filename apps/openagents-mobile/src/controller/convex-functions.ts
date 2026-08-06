import { makeFunctionReference } from "convex/server";

import type { AttentionShell, AggregateType, WorkDetail, WorkShell } from "./contracts";

export const attentionInboxQuery = makeFunctionReference<
  "query",
  { limit?: number },
  ReadonlyArray<AttentionShell>
>("workShells:attentionInbox");

export const workDetailsQuery = makeFunctionReference<
  "query",
  { aggregateType: AggregateType; aggregateId: string; limit?: number },
  ReadonlyArray<WorkDetail>
>("workShells:listDetails");

export const workShellQuery = makeFunctionReference<
  "query",
  { aggregateType: AggregateType; aggregateId: string },
  WorkShell | null
>("workShells:get");
