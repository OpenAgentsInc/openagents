import { UIMessage } from '../chat/types';
import { RxCollection, RxDatabase, RxDocument } from 'rxdb';
import { DeepReadonlyObject } from '../types';
import { v4 as uuidv4 } from 'uuid';

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
  id?: string;
  threadId: string;
  role: string;
  content: string;
  createdAt: number;
  parts: any[];
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
  // destroy() is already provided by RxDatabase
}

/**
 * Factory function to convert stored message to UIMessage
 */
export function storedMessageToUIMessage(storedMessage: StoredMessage | DeepReadonlyObject<StoredMessage>): UIMessage {
  // Create a mutable copy of the parts array
  const parts = Array.isArray(storedMessage.parts)
    ? storedMessage.parts.map(part => ({ ...part }))
    : [];

  // Create a mutable copy of attachments if they exist
  const attachments = storedMessage.attachments
    ? storedMessage.attachments.map(attachment => ({ ...attachment }))
    : undefined;

  return {
    id: storedMessage.id || uuidv4(),
    role: storedMessage.role as 'user' | 'assistant' | 'system' | 'data',
    content: storedMessage.content,
    threadId: storedMessage.threadId,
    createdAt: new Date(storedMessage.createdAt),
    parts,
    ...(attachments ? { experimental_attachments: attachments } : {})
  };
}

/**
 * Factory function to convert UIMessage to stored message
 */
export function uiMessageToStoredMessage(message: UIMessage, threadId: string): StoredMessage {
  return {
    id: message.id || uuidv4(),
    threadId,
    role: message.role,
    content: message.content,
    createdAt: message.createdAt?.getTime() || Date.now(),
    parts: message.parts?.map(part => ({ ...part })) || [],
    // Always provide an empty array instead of null/undefined for attachments
    attachments: message.experimental_attachments?.map(attachment => ({ ...attachment })) || []
  };
}
