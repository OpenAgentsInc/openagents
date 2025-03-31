import { Message as VercelMessage } from 'ai/react';
import { v4 as uuidv4 } from 'uuid';
import { DeepReadonlyObject } from '../types';

export type IdGenerator = () => string;

/**
 * Fetch function type (standardizes the version of fetch used).
 */
export type FetchFunction = typeof globalThis.fetch;

/**
Reason why a language model finished generating a response.

Can be one of the following:
- `stop`: model generated stop sequence
- `length`: model generated maximum number of tokens
- `content-filter`: content filter violation stopped the model
- `tool-calls`: model triggered tool calls
- `error`: model stopped because of an error
- `other`: model stopped for other reasons
- `unknown`: the model has not transmitted a finish reason
 */
export type LanguageModelV1FinishReason =
  | 'stop' // model generated stop sequence
  | 'length' // model generated maximum number of tokens
  | 'content-filter' // content filter violation stopped the model
  | 'tool-calls' // model triggered tool calls
  | 'error' // model stopped because of an error
  | 'other' // model stopped for other reasons
  | 'unknown'; // the model has not transmitted a finish reason


/**
Represents the number of tokens used in a prompt and completion.
 */
export type LanguageModelUsage = {
  /**
The number of tokens used in the prompt.
   */
  promptTokens: number;

  /**
The number of tokens used in the completion.
 */
  completionTokens: number;

  /**
The total number of tokens used (promptTokens + completionTokens).
   */
  totalTokens: number;
};

/**
 * A source that has been used as input to generate the response.
 */
export type LanguageModelV1Source = {
  /**
   * A URL source. This is return by web search RAG models.
   */
  sourceType: 'url';

  /**
   * The ID of the source.
   */
  id: string;

  /**
   * The URL of the source.
   */
  url: string;

  /**
   * The title of the source.
   */
  title?: string;

  /**
   * Additional provider metadata for the source.
   */
  providerMetadata?: LanguageModelV1ProviderMetadata;
};


/**
 * Additional provider-specific metadata. They are passed through
 * to the provider from the AI SDK and enable provider-specific
 * functionality that can be fully encapsulated in the provider.
 *
 * This enables us to quickly ship provider-specific functionality
 * without affecting the core AI SDK.
 *
 * The outer record is keyed by the provider name, and the inner
 * record is keyed by the provider-specific metadata key.
 *
 * ```ts
 * {
 *   "anthropic": {
 *     "cacheControl": { "type": "ephemeral" }
 *   }
 * }
 * ```
 */
// TODO language model v2 separate provider metadata (output) from provider options (input)
export type LanguageModelV1ProviderMetadata = Record<
  string,
  Record<string, JSONValue>
>;


/**
Typed tool call that is returned by generateText and streamText.
It contains the tool call ID, the tool name, and the tool arguments.
 */
export interface ToolCall<NAME extends string, ARGS> {
  /**
ID of the tool call. This ID is used to match the tool call with the tool result.
 */
  toolCallId: string;

  /**
Name of the tool that is being called.
 */
  toolName: NAME;

  /**
Arguments of the tool call. This is a JSON-serializable object that matches the tool's input schema.
   */
  args: ARGS;
}

/**
 * @deprecated Use `ToolCall` instead.
 */
// TODO remove in v5
export type CoreToolCall<NAME extends string, ARGS> = ToolCall<NAME, ARGS>;

/**
Typed tool result that is returned by `generateText` and `streamText`.
It contains the tool call ID, the tool name, the tool arguments, and the tool result.
 */
export interface ToolResult<NAME extends string, ARGS, RESULT> {
  /**
ID of the tool call. This ID is used to match the tool call with the tool result.
   */
  toolCallId: string;

  /**
Name of the tool that was called.
   */
  toolName: NAME;

  /**
Arguments of the tool call. This is a JSON-serializable object that matches the tool's input schema.
     */
  args: ARGS;

  /**
Result of the tool call. This is the result of the tool's execution.
     */
  result: RESULT;
}

/**
 * @deprecated Use `ToolResult` instead.
 */
// TODO remove in v5
export type CoreToolResult<NAME extends string, ARGS, RESULT> = ToolResult<
  NAME,
  ARGS,
  RESULT
>;


/**
Tool invocations are either tool calls or tool results. For each assistant tool call,
there is one tool invocation. While the call is in progress, the invocation is a tool call.
Once the call is complete, the invocation is a tool result.

The step is used to track how to map an assistant UI message with many tool invocations
back to a sequence of LLM assistant/tool result message pairs.
It is optional for backwards compatibility.
 */
export type ToolInvocation =
  | ({ state: 'partial-call'; step?: number } & ToolCall<string, any>)
  | ({ state: 'call'; step?: number } & ToolCall<string, any>)
  | ({ state: 'result'; step?: number } & ToolResult<string, any, any>);

/**
 * An attachment that can be sent along with a message.
 */
export interface Attachment {
  /**
   * The name of the attachment, usually the file name.
   */
  name?: string;

  /**
   * A string indicating the [media type](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Type).
   * By default, it's extracted from the pathname's extension.
   */
  contentType?: string;

  /**
   * The URL of the attachment. It can either be a URL to a hosted file or a [Data URL](https://developer.mozilla.org/en-US/docs/Web/HTTP/Basics_of_HTTP/Data_URLs).
   */
  url: string;
}

/**
 * AI SDK UI Messages. They are used in the client and to communicate between the frontend and the API routes.
 */
export interface Message {
  /**
A unique identifier for the message.
   */
  id: string;

  /**
The timestamp of the message.
   */
  createdAt?: Date;

  /**
Text content of the message. Use parts when possible.
   */
  content: string;

  /**
Reasoning for the message.

@deprecated Use `parts` instead.
   */
  reasoning?: string;

  /**
   * Additional attachments to be sent along with the message.
   */
  experimental_attachments?: Attachment[];

  /**
The 'data' role is deprecated.
   */
  role: 'system' | 'user' | 'assistant' | 'data';

  /**
For data messages.

@deprecated Data messages will be removed.
   */
  data?: JSONValue;

  /**
   * Additional message-specific information added on the server via StreamData
   */
  annotations?: JSONValue[] | undefined;

  /**
Tool invocations (that can be tool calls or tool results, depending on whether or not the invocation has finished)
that the assistant made as part of this message.

@deprecated Use `parts` instead.
   */
  toolInvocations?: Array<ToolInvocation>;

  /**
   * The parts of the message. Use this for rendering the message in the UI.
   *
   * Assistant messages can have text, reasoning and tool invocation parts.
   * User messages can have text parts.
   */
  // note: optional on the Message type (which serves as input)
  parts?: Array<
    | TextUIPart
    | ReasoningUIPart
    | ToolInvocationUIPart
    | SourceUIPart
    | FileUIPart
    | StepStartUIPart
  >;
}

/**
 * A step start part of a message.
 *
 * Matching the official @ai-sdk/ui-utils StepStartUIPart definition
 * which doesn't require the 'step' property.
 */
export type StepStartUIPart = {
  type: 'step-start';
  step?: number; // Make step optional to match the ai-sdk definition
};

/**
 * Base message part types
 */
export type BasePart = TextUIPart | ReasoningUIPart | ToolInvocationUIPart | SourceUIPart | FileUIPart;

/**
 * UI-specific part types
 */
export type UIPart = BasePart | StepStartUIPart;

/**
 * Base message interface that extends Vercel's Message type but makes parts optional
 */
export interface BaseMessage extends Omit<VercelMessage, 'parts'> {
  parts?: BasePart[];
}

/**
 * Extended UI message interface that adds our custom functionality
 */
export interface UIMessage {
  id: string;
  role: 'system' | 'data' | 'user' | 'assistant';
  content: string;
  createdAt?: Date;
  threadId?: string;
  parts: UIPart[];
  experimental_attachments?: any[];
  toolInvocations?: ToolInvocation[];
}

/**
 * Convert UIMessage to Vercel Message
 */
export function toVercelMessage(message: UIMessage): BaseMessage {
  const parts = message.parts.filter(part =>
    part.type !== 'step-start'
  ) as BasePart[];

  return {
    id: message.id || uuidv4(),
    role: message.role,
    content: message.content,
    parts
  };
}

/**
 * Convert Vercel Message to UIMessage
 */
export function fromVercelMessage(message: VercelMessage): UIMessage {
  return {
    id: message.id || uuidv4(),
    role: message.role,
    content: message.content,
    createdAt: new Date(),
    parts: (message.parts || []) as UIPart[],
    experimental_attachments: []  // Always provide an empty array for attachments
  };
}

/**
 * A text part of a message.
 */
export type TextUIPart = {
  type: 'text';

  /**
   * The text content.
   */
  text: string;
};

/**
 * A reasoning part of a message.
 */
export type ReasoningUIPart = {
  type: 'reasoning';

  /**
   * The reasoning text.
   */
  // TODO: v5 rename to `text`
  reasoning: string;

  details: Array<
    | { type: 'text'; text: string; signature?: string }
    | { type: 'redacted'; data: string }
  >;
};

/**
 * A tool invocation part of a message.
 */
export type ToolInvocationUIPart = {
  type: 'tool-invocation';

  /**
   * The tool invocation.
   */
  toolInvocation: ToolInvocation;
};

/**
 * A source part of a message.
 */
export type SourceUIPart = {
  type: 'source';

  /**
   * The source.
   */
  source: LanguageModelV1Source;
};

/**
 * A file part of a message.
 */
export type FileUIPart = {
  type: 'file';
  mimeType: string;
  data: string; // base64 encoded data
};

export type CreateMessage = Omit<Message, 'id'> & {
  id?: Message['id'];
};

export type ChatRequest = {
  /**
An optional object of headers to be passed to the API endpoint.
 */
  headers?: Record<string, string> | Headers;

  /**
An optional object to be passed to the API endpoint.
*/
  body?: object;

  /**
The messages of the chat.
   */
  messages: Message[];

  /**
Additional data to be sent to the server.
   */
  data?: JSONValue;
};

// Note: only used in useCompletion
export type RequestOptions = {
  /**
An optional object of headers to be passed to the API endpoint.
 */
  headers?: Record<string, string> | Headers;

  /**
An optional object to be passed to the API endpoint.
   */
  body?: object;
};

export type ChatRequestOptions = {
  /**
Additional headers that should be to be passed to the API endpoint.
 */
  headers?: Record<string, string> | Headers;

  /**
Additional body JSON properties that should be sent to the API endpoint.
 */
  body?: object;

  /**
Additional data to be sent to the API endpoint.
   */
  data?: JSONValue;

  /**
   * Additional files to be sent to the server.
   */
  experimental_attachments?: FileList | Array<Attachment>;

  /**
   * Allow submitting an empty message. Defaults to `false`.
   */
  allowEmptySubmit?: boolean;
};

export type UseChatOptions = {
  /**
Keeps the last message when an error happens. Defaults to `true`.

@deprecated This option will be removed in the next major release.
   */
  keepLastMessageOnError?: boolean;

  /**
   * The API endpoint that accepts a `{ messages: Message[] }` object and returns
   * a stream of tokens of the AI chat response. Defaults to `/api/chat`.
   */
  api?: string;

  /**
   * A unique identifier for the chat. If not provided, a random one will be
   * generated. When provided, the `useChat` hook with the same `id` will
   * have shared states across components.
   */
  id?: string;

  /**
   * Initial messages of the chat. Useful to load an existing chat history.
   */
  initialMessages?: Message[];

  /**
   * Initial input of the chat.
   */
  initialInput?: string;

  /**
Optional callback function that is invoked when a tool call is received.
Intended for automatic client-side tool execution.

You can optionally return a result for the tool call,
either synchronously or asynchronously.
   */
  onToolCall?: ({
    toolCall,
  }: {
    toolCall: ToolCall<string, unknown>;
  }) => void | Promise<unknown> | unknown;

  /**
   * Callback function to be called when the API response is received.
   */
  onResponse?: (response: Response) => void | Promise<void>;

  /**
   * Optional callback function that is called when the assistant message is finished streaming.
   *
   * @param message The message that was streamed.
   * @param options.usage The token usage of the message.
   * @param options.finishReason The finish reason of the message.
   */
  onFinish?: (
    message: Message,
    options: {
      usage: LanguageModelUsage;
      finishReason: LanguageModelV1FinishReason;
    },
  ) => void;

  /**
   * Callback function to be called when an error is encountered.
   */
  onError?: (error: Error) => void;

  /**
   * A way to provide a function that is going to be used for ids for messages and the chat.
   * If not provided the default AI SDK `generateId` is used.
   */
  generateId?: IdGenerator;

  /**
   * The credentials mode to be used for the fetch request.
   * Possible values are: 'omit', 'same-origin', 'include'.
   * Defaults to 'same-origin'.
   */
  credentials?: RequestCredentials;

  /**
   * HTTP headers to be sent with the API request.
   */
  headers?: Record<string, string> | Headers;

  /**
   * Extra body object to be sent with the API request.
   * @example
   * Send a `sessionId` to the API along with the messages.
   * ```js
   * useChat({
   *   body: {
   *     sessionId: '123',
   *   }
   * })
   * ```
   */
  body?: object;

  /**
   * Whether to send extra message fields such as `message.id` and `message.createdAt` to the API.
   * Defaults to `false`. When set to `true`, the API endpoint might need to
   * handle the extra fields before forwarding the request to the AI service.
   */
  sendExtraMessageFields?: boolean;

  /**
Streaming protocol that is used. Defaults to `data`.
   */
  streamProtocol?: 'data' | 'text';

  /**
Custom fetch implementation. You can use it as a middleware to intercept requests,
or to provide a custom fetch implementation for e.g. testing.
    */
  fetch?: FetchFunction;
};

export type UseCompletionOptions = {
  /**
   * The API endpoint that accepts a `{ prompt: string }` object and returns
   * a stream of tokens of the AI completion response. Defaults to `/api/completion`.
   */
  api?: string;
  /**
   * An unique identifier for the chat. If not provided, a random one will be
   * generated. When provided, the `useChat` hook with the same `id` will
   * have shared states across components.
   */
  id?: string;

  /**
   * Initial prompt input of the completion.
   */
  initialInput?: string;

  /**
   * Initial completion result. Useful to load an existing history.
   */
  initialCompletion?: string;

  /**
   * Callback function to be called when the API response is received.
   */
  onResponse?: (response: Response) => void | Promise<void>;

  /**
   * Callback function to be called when the completion is finished streaming.
   */
  onFinish?: (prompt: string, completion: string) => void;

  /**
   * Callback function to be called when an error is encountered.
   */
  onError?: (error: Error) => void;

  /**
   * The credentials mode to be used for the fetch request.
   * Possible values are: 'omit', 'same-origin', 'include'.
   * Defaults to 'same-origin'.
   */
  credentials?: RequestCredentials;

  /**
   * HTTP headers to be sent with the API request.
   */
  headers?: Record<string, string> | Headers;

  /**
   * Extra body object to be sent with the API request.
   * @example
   * Send a `sessionId` to the API along with the prompt.
   * ```js
   * useChat({
   *   body: {
   *     sessionId: '123',
   *   }
   * })
   * ```
   */
  body?: object;

  /**
Streaming protocol that is used. Defaults to `data`.
   */
  streamProtocol?: 'data' | 'text';

  /**
Custom fetch implementation. You can use it as a middleware to intercept requests,
or to provide a custom fetch implementation for e.g. testing.
    */
  fetch?: FetchFunction;
};

/**
A JSON value can be a string, number, boolean, object, array, or null.
JSON values can be serialized and deserialized by the JSON.stringify and JSON.parse methods.
 */
export type JSONValue =
  | null
  | string
  | number
  | boolean
  | { [value: string]: JSONValue }
  | Array<JSONValue>;

export type AssistantMessage = {
  id: string;
  role: 'assistant';
  content: Array<{
    type: 'text';
    text: {
      value: string;
    };
  }>;
};

/*
 * A data message is an application-specific message from the assistant
 * that should be shown in order with the other messages.
 *
 * It can trigger other operations on the frontend, such as annotating
 * a map.
 */
export type DataMessage = {
  id?: string; // optional id, implement if needed (e.g. for persistance)
  role: 'data';
  data: JSONValue; // application-specific data
};
