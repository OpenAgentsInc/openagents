export type ExternalStoreAdapter = unknown;
export type AppendMessage = unknown;
export type AUIThreadMessageLike = {
  id: string | number;
  role: 'user' | 'assistant';
  createdAt?: Date;
  content: ReadonlyArray<any>;
};

export class ExportedMessageRepository {}

export function useExternalStoreRuntime(): unknown {
  return {};
}

