import { UIMessage } from '../chat/types';
import { RxCollection, RxDatabase, RxDocument } from 'rxdb';

/**
 * Thread data object
 */
export interface Thread {
  id: string;
  title: string;
  createdAt: number;
  updatedAt: number;
  modelId?: string;
  systemPrompt?: string;
  metadata?: Record<string, any>;
}

/**
 * Thread document (as stored in RxDB)
 */
export type ThreadDocument = RxDocument<Thread>;

/**
 * Message data object (for database storage)
 */
export interface StoredMessage {
  id: string;
  threadId: string;
  role: string;
  content: string;
  createdAt: number;
  parts?: any[];
  attachments?: any[];
}

/**
 * Message document (as stored in RxDB)
 */
export type MessageDocument = RxDocument<StoredMessage>;

/**
 * Settings data object
 */
export interface Settings {
  id: string;
  theme?: string;
  apiKeys?: Record<string, string>;
  defaultModel?: string;
  preferences?: Record<string, any>;
}

/**
 * Settings document (as stored in RxDB)
 */
export type SettingsDocument = RxDocument<Settings>;

/**
 * Database collections
 */
export interface DatabaseCollections {
  threads: RxCollection<Thread>;
  messages: RxCollection<StoredMessage>;
  settings: RxCollection<Settings>;
}

/**
 * RxDB database with our collections
 */
export interface Database extends RxDatabase<DatabaseCollections> {
  destroy: () => Promise<void>;
}

// Type to represent a deep readonly object (RxDB returns readonly objects)
export type DeepReadonlyObject<T> = {
  readonly [K in keyof T]: T[K] extends object ? DeepReadonlyObject<T[K]> : T[K];
};

/**
 * Factory function to convert stored message to UIMessage
 */
export function storedMessageToUIMessage(storedMessage: StoredMessage | DeepReadonlyObject<StoredMessage>): UIMessage {
  return {
    id: storedMessage.id,
    role: storedMessage.role as any,
    content: storedMessage.content,
    createdAt: new Date(storedMessage.createdAt),
    parts: Array.isArray(storedMessage.parts) ? [...storedMessage.parts] : [],
    ...(storedMessage.attachments ? {
      experimental_attachments: Array.isArray(storedMessage.attachments) ? [...storedMessage.attachments] : []
    } : {})
  } as UIMessage;
}

/**
 * Factory function to convert UIMessage to stored message
 */
export function uiMessageToStoredMessage(message: UIMessage, threadId: string): StoredMessage {
  return {
    id: message.id,
    threadId,
    role: message.role,
    content: message.content,
    createdAt: message.createdAt ? message.createdAt.getTime() : Date.now(),
    parts: Array.isArray(message.parts) ? [...message.parts] : [],
    attachments: Array.isArray(message.experimental_attachments) ? [...message.experimental_attachments] : []
  };
}
