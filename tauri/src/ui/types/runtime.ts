import type { ThreadMessageLike } from "./thread";

export type ExternalStoreAdapter<T = ThreadMessageLike> = {
  isRunning?: boolean;
  isLoading?: boolean;
  messages: readonly T[];
  convertMessage?: (message: T, idx: number) => ThreadMessageLike;
  onNew: (message: {
    role: "user";
    content: readonly any[]; // user parts only in our usage
    parentId: string | null;
    attachments?: any[];
  }) => Promise<void>;
};

export type AttachmentAdapter = {
  accept: string;
  add: (o: { file: File }) => Promise<any>;
  send: (attachment: any) => Promise<any>;
  remove: (attachment: any) => Promise<void>;
};

