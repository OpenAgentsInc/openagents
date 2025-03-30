import { Message as VercelMessage } from 'ai/react';
import { DeepReadonlyObject } from '../../types';

export type Role = 'user' | 'assistant' | 'system' | 'data';

export type MessagePart = {
  type: string;
  text?: string;
  [key: string]: any;
};

export interface UIMessage {
  id?: string;
  role: Role;
  content: string;
  threadId?: string;
  createdAt?: number;
  parts?: MessagePart[];
}
