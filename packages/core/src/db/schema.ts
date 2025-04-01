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
    id: {
      type: 'string',
      maxLength: 100
    },
    title: {
      type: 'string',
      maxLength: 500
    },
    createdAt: {
      type: 'number',
      // Removed multipleOf constraint to allow millisecond precision
      minimum: 0,
      maximum: 9007199254740991 // Number.MAX_SAFE_INTEGER
    },
    updatedAt: {
      type: 'number',
      // Removed multipleOf constraint to allow millisecond precision
      minimum: 0,
      maximum: 9007199254740991 // Number.MAX_SAFE_INTEGER
    },
    modelId: {
      type: 'string',
      maxLength: 100
    },
    systemPrompt: {
      type: 'string',
      maxLength: 2000
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
    id: {
      type: 'string',
      maxLength: 100
    },
    threadId: {
      type: 'string',
      maxLength: 100
    },
    role: {
      type: 'string',
      enum: ['user', 'assistant', 'system']
    },
    content: {
      type: 'string',
      maxLength: 100000
    },
    createdAt: {
      type: 'number',
      // Removed multipleOf constraint to allow millisecond precision
      minimum: 0,
      maximum: 9007199254740991 // Number.MAX_SAFE_INTEGER
    },
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
    id: {
      type: 'string',
      maxLength: 100
    },
    theme: {
      type: 'string',
      enum: ['light', 'dark', 'system']
    },
    apiKeys: {
      type: 'object',
      additionalProperties: true
    },
    defaultModel: {
      type: 'string',
      maxLength: 100
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
