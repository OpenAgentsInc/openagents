import { RxJsonSchema } from 'rxdb';
import { Thread, StoredMessage, Settings } from './types';

/**
 * Thread collection schema
 */
export const threadSchema: RxJsonSchema<Thread> = {
  version: 0,
  title: 'thread schema',
  primaryKey: 'id',
  type: 'object',
  properties: {
    id: { type: 'string' },
    title: { type: 'string' },
    createdAt: { type: 'number' },
    updatedAt: { type: 'number' },
    modelId: { 
      type: 'string' 
    },
    systemPrompt: { 
      type: 'string' 
    },
    metadata: { 
      type: 'object',
      additionalProperties: true
    }
  },
  required: ['id', 'title', 'createdAt', 'updatedAt'],
  indexes: ['updatedAt']
};

/**
 * Message collection schema
 */
export const messageSchema: RxJsonSchema<StoredMessage> = {
  version: 0,
  title: 'message schema',
  primaryKey: 'id',
  type: 'object',
  properties: {
    id: { type: 'string' },
    threadId: { type: 'string' },
    role: { type: 'string' },
    content: { type: 'string' },
    createdAt: { type: 'number' },
    parts: { 
      type: 'array',
      items: {
        type: 'object',
        additionalProperties: true
      }
    },
    attachments: { 
      type: 'array',
      items: {
        type: 'object',
        additionalProperties: true
      }
    }
  },
  required: ['id', 'threadId', 'role', 'content', 'createdAt'],
  indexes: ['threadId', 'createdAt']
};

/**
 * Settings collection schema
 */
export const settingsSchema: RxJsonSchema<Settings> = {
  version: 0,
  title: 'settings schema',
  primaryKey: 'id',
  type: 'object',
  properties: {
    id: { type: 'string' },
    theme: { 
      type: 'string' 
    },
    apiKeys: { 
      type: 'object',
      additionalProperties: true
    },
    defaultModel: { 
      type: 'string' 
    },
    preferences: { 
      type: 'object',
      additionalProperties: true
    }
  },
  required: ['id']
};

/**
 * All schemas for export
 */
export const schemas = {
  threadSchema,
  messageSchema,
  settingsSchema
};