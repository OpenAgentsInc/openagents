import { decodeKhalaRuntimeControlIntent, decodePushRequest } from "@openagentsinc/khala-sync";
import {
  CHAT_APPEND_MESSAGE_MUTATOR_NAME,
  CHAT_BIND_THREAD_REPO_MUTATOR_NAME,
  CHAT_CREATE_THREAD_MUTATOR_NAME,
  RUNTIME_START_TURN_MUTATOR_NAME,
  executePush,
  type MutatorRegistry,
  type SyncSql,
} from "@openagentsinc/khala-sync-server";
import { Schema as S } from "effect";

export class CloudCodingRuntimeSeedError extends S.TaggedErrorClass<CloudCodingRuntimeSeedError>()(
  "CloudCodingRuntimeSeedError",
  { reason: S.String },
) {}

type ExistingTurnRow = Readonly<{
  owner_user_id: string;
  thread_id: string;
}>;

export const seedCloudCodingRuntimeTurn = async (
  input: Readonly<{
    branch: string;
    objective: string;
    ownerUserId: string;
    registry: MutatorRegistry;
    repositoryFullName: string;
    sql: SyncSql;
    threadRef: string;
    turnId: string;
    nowIso: string;
    executePushImpl?: typeof executePush;
  }>,
): Promise<void> => {
  const existing = (await input.sql`
    SELECT owner_user_id, thread_id
    FROM khala_sync_runtime_turns
    WHERE turn_id = ${input.turnId}
    LIMIT 1
  `) as Array<ExistingTurnRow>;
  if (existing[0] !== undefined) {
    if (
      existing[0].owner_user_id !== input.ownerUserId ||
      existing[0].thread_id !== input.threadRef
    ) {
      throw new CloudCodingRuntimeSeedError({
        reason: "cloud_coding_runtime_turn_scope_mismatch",
      });
    }
    return;
  }

  const [repositoryOwner, repositoryName] = input.repositoryFullName.split("/");
  if (repositoryOwner === undefined || repositoryName === undefined) {
    throw new CloudCodingRuntimeSeedError({
      reason: "cloud_coding_repository_ref_invalid",
    });
  }
  const messageRef = `message.cloud-coding.${input.turnId}`;
  const intent = decodeKhalaRuntimeControlIntent({
    bodyRef: `chat_message.${messageRef}`,
    causalityRefs: [input.threadRef],
    createdAt: input.nowIso,
    idempotencyKey: `idempotency.cloud-coding.${input.turnId}`,
    intentId: `intent.cloud-coding.${input.turnId}`,
    kind: "turn.start",
    origin: { lane: "hosted_khala", surface: "server" },
    redactionClass: "private_ref",
    schema: "openagents.khala_runtime_control_intent.v1",
    target: { adapterKind: "openagents_native", lane: "managed_cloud" },
    threadId: input.threadRef,
    turnId: input.turnId,
    visibility: "private",
  });
  const response = await (input.executePushImpl ?? executePush)({
    registry: input.registry,
    request: decodePushRequest({
      clientGroupId: `server.cloud-coding.${input.ownerUserId}`,
      clientId: `server.cloud-coding.${input.turnId}`,
      mutations: [
        {
          argsJson: JSON.stringify({
            threadId: input.threadRef,
            title: "Omega Agent Computer task",
          }),
          mutationId: 1,
          name: CHAT_CREATE_THREAD_MUTATOR_NAME,
        },
        {
          argsJson: JSON.stringify({
            repo: {
              defaultBranch: input.branch,
              name: repositoryName,
              owner: repositoryOwner,
            },
            threadId: input.threadRef,
          }),
          mutationId: 2,
          name: CHAT_BIND_THREAD_REPO_MUTATOR_NAME,
        },
        {
          argsJson: JSON.stringify({
            attachments: [],
            body: input.objective,
            messageId: messageRef,
            threadId: input.threadRef,
          }),
          mutationId: 3,
          name: CHAT_APPEND_MESSAGE_MUTATOR_NAME,
        },
        {
          argsJson: JSON.stringify(intent),
          mutationId: 4,
          name: RUNTIME_START_TURN_MUTATOR_NAME,
        },
      ],
      protocolVersion: 1,
      schemaVersion: 1,
    }),
    sql: input.sql,
    userId: input.ownerUserId,
  });
  const rejected = response.results.find((result) => result.status !== "applied");
  if (rejected !== undefined) {
    throw new CloudCodingRuntimeSeedError({
      reason:
        rejected.errorCode === undefined
          ? "cloud_coding_runtime_seed_rejected"
          : `cloud_coding_runtime_seed_rejected.${rejected.errorCode}`,
    });
  }
};
