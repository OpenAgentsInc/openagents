import { RxJsonSchema } from 'rxdb';
import type { Thread, StoredMessage, Settings } from './types';

// Increment this when making schema changes
// This allows RxDB to handle migrations properly
const SCHEMA_VERSION = 3;

/**
 * Thread collection schema
 */
export const threadSchema: RxJsonSchema<Thread> = {
  version: SCHEMA_VERSION,
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
      multipleOf: 0.001, // Allow millisecond precision (0.001 = 1ms)
      minimum: 0,
      maximum: 9007199254740991 // Number.MAX_SAFE_INTEGER
    },
    updatedAt: {
      type: 'number',
      multipleOf: 0.001, // Allow millisecond precision (0.001 = 1ms)
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
  version: SCHEMA_VERSION,
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
      multipleOf: 0.001, // Allow millisecond precision (0.001 = 1ms)
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
  version: SCHEMA_VERSION,
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
    selectedModelId: {
      type: 'string',
      maxLength: 100
    },
    visibleModelIds: {
      type: 'array',
      items: {
        type: 'string',
        maxLength: 100
      }
    },
    preferences: {
      type: 'object',
      additionalProperties: true
    },
    mcpClients: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          id: { type: 'string' },
          name: { type: 'string' },
          enabled: { type: 'boolean' },
          type: { type: 'string', enum: ['sse', 'stdio'] },
          url: { type: 'string' },
          command: { type: 'string' },
          args: {
            type: 'array',
            items: { type: 'string' }
          },
          env: {
            type: 'object',
            additionalProperties: true
          },
          lastConnected: { type: 'number' },
          status: {
            type: 'string',
            enum: ['connected', 'disconnected', 'error']
          },
          statusMessage: { type: 'string' }
        },
        required: ['id', 'name', 'enabled', 'type']
      }
    },
    enabledToolIds: {
      type: 'array',
      items: {
        type: 'string',
        maxLength: 100
      }
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
