type SqlValue = null | boolean | number | string | ArrayBuffer | ArrayBufferView

interface D1Meta {
  readonly changed_db?: boolean
  readonly changes: number
  readonly duration?: number
  readonly last_row_id?: number
  readonly rows_read?: number
  readonly rows_written?: number
  readonly served_by?: string
  readonly size_after?: number
}

interface D1Result<T = Record<string, SqlValue>> {
  readonly error?: string
  readonly meta: D1Meta
  readonly results: Array<T>
  readonly success: boolean
}

interface D1ExecResult {
  readonly count: number
  readonly duration: number
}

interface D1PreparedStatement {
  all<T = Record<string, SqlValue>>(): Promise<D1Result<T>>
  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement
  first<T = Record<string, SqlValue>>(columnName?: string): Promise<T | null>
  raw<T = ReadonlyArray<SqlValue>>(options?: Readonly<{ columnNames?: boolean }>): Promise<Array<T>>
  run<T = Record<string, SqlValue>>(): Promise<D1Result<T>>
}

interface D1DatabaseSession {
  batch<T = Record<string, SqlValue>>(
    statements: ReadonlyArray<D1PreparedStatement>,
  ): Promise<Array<D1Result<T>>>
  getBookmark(): string | null
  prepare(query: string): D1PreparedStatement
}

interface D1Database {
  batch<T = Record<string, SqlValue>>(
    statements: ReadonlyArray<D1PreparedStatement>,
  ): Promise<Array<D1Result<T>>>
  dump(): Promise<ArrayBuffer>
  exec(query: string): Promise<D1ExecResult>
  prepare(query: string): D1PreparedStatement
  withSession(bookmark?: string): D1DatabaseSession
}

type R2PutValue = ArrayBuffer | ArrayBufferView | Blob | ReadableStream | string | null

interface R2HTTPMetadata {
  cacheControl?: string
  cacheExpiry?: Date
  contentDisposition?: string
  contentEncoding?: string
  contentLanguage?: string
  contentType?: string
}

interface R2Checksums {
  md5?: ArrayBuffer
  sha1?: ArrayBuffer
  sha256?: ArrayBuffer
  sha384?: ArrayBuffer
  sha512?: ArrayBuffer
}

interface R2Object {
  readonly checksums: R2Checksums
  readonly customMetadata?: Record<string, string>
  readonly etag: string
  readonly httpEtag: string
  readonly httpMetadata?: R2HTTPMetadata
  readonly key: string
  readonly size: number
  readonly uploaded: Date
  readonly version: string
  writeHttpMetadata(headers: Headers): void
}

interface R2ObjectBody extends R2Object {
  readonly body: ReadableStream
  readonly bodyUsed: boolean
  arrayBuffer(): Promise<ArrayBuffer>
  blob(): Promise<Blob>
  json<T = unknown>(): Promise<T>
  text(): Promise<string>
}

interface R2PutOptions {
  customMetadata?: Record<string, string>
  httpMetadata?: R2HTTPMetadata | Headers
  onlyIf?: unknown
  sha1?: ArrayBuffer | string
  sha256?: ArrayBuffer | string
}

interface R2ListOptions {
  cursor?: string
  delimiter?: string
  include?: ReadonlyArray<'httpMetadata' | 'customMetadata'>
  limit?: number
  prefix?: string
  startAfter?: string
}

interface R2Objects {
  readonly cursor?: string
  readonly delimitedPrefixes: Array<string>
  readonly objects: Array<R2Object>
  readonly truncated: boolean
}

interface R2Bucket {
  createMultipartUpload(key: string, options?: R2PutOptions): Promise<R2MultipartUpload>
  delete(keys: string | ReadonlyArray<string>): Promise<void>
  get(key: string, options?: unknown): Promise<R2ObjectBody | null>
  head(key: string): Promise<R2Object | null>
  list(options?: R2ListOptions): Promise<R2Objects>
  put(key: string, value: R2PutValue, options?: R2PutOptions): Promise<R2Object>
}

interface R2MultipartUpload {
  readonly key: string
  readonly uploadId: string
  abort(): Promise<void>
  complete(uploadedParts: ReadonlyArray<Readonly<{ etag: string; partNumber: number }>>): Promise<R2Object>
  uploadPart(partNumber: number, value: R2PutValue): Promise<Readonly<{ etag: string; partNumber: number }>>
}

interface Fetcher {
  connect(address: string | Readonly<{ hostname: string; port: number }>, options?: unknown): unknown
  fetch(input: RequestInfo | URL, init?: RequestInit): Promise<Response>
}

interface DurableObjectId {
  readonly name?: string
  equals(other: DurableObjectId): boolean
  toString(): string
}

interface DurableObjectStub extends Fetcher {
  readonly id: DurableObjectId
  readonly name?: string
}

interface DurableObjectNamespace {
  get(id: DurableObjectId, options?: unknown): DurableObjectStub
  getByName(name: string, options?: unknown): DurableObjectStub
  idFromName(name: string): DurableObjectId
  idFromString(id: string): DurableObjectId
  jurisdiction(name: string): DurableObjectNamespace
  newUniqueId(options?: unknown): DurableObjectId
}

interface DurableObjectStorageSql {
  exec<T = Record<string, SqlValue>>(
    query: string,
    ...bindings: ReadonlyArray<unknown>
  ): Readonly<{ columnNames: Array<string>; rowsRead: number; rowsWritten: number; toArray(): Array<T> }>
}

interface DurableObjectStorage {
  readonly sql: DurableObjectStorageSql
  delete(key: string | ReadonlyArray<string>): Promise<boolean | number>
  deleteAll(): Promise<void>
  get<T = unknown>(key: string): Promise<T | undefined>
  get<T = unknown>(keys: ReadonlyArray<string>): Promise<Map<string, T>>
  getAlarm(): Promise<number | null>
  list<T = unknown>(options?: unknown): Promise<Map<string, T>>
  put<T>(key: string, value: T): Promise<void>
  put<T>(entries: Record<string, T>): Promise<void>
  setAlarm(scheduledTime: number | Date): Promise<void>
  transaction<T>(closure: (transaction: DurableObjectStorage) => Promise<T>): Promise<T>
  transactionSync<T>(closure: () => T): T
}

interface DurableObjectState {
  readonly id: DurableObjectId
  readonly storage: DurableObjectStorage
  acceptWebSocket(socket: WebSocket, tags?: ReadonlyArray<string>): void
  blockConcurrencyWhile<T>(callback: () => Promise<T>): Promise<T>
  getWebSockets(tag?: string): Array<WebSocket>
  waitUntil(promise: Promise<unknown>): void
}

interface WebSocket {
  deserializeAttachment(): unknown
  serializeAttachment(attachment: unknown): void
}

declare class WebSocketPair {
  readonly 0: WebSocket
  readonly 1: WebSocket
}

interface ExecutionContext {
  readonly props: unknown
  readonly tracing: unknown
  passThroughOnException(): void
  waitUntil(promise: Promise<unknown>): void
}

interface ResponseInit {
  cf?: unknown
  encodeBody?: 'automatic' | 'manual'
  webSocket?: WebSocket | null
}

interface Response {
  json<T = unknown>(): Promise<T>
}

interface ScheduledController {
  readonly cron: string
  readonly scheduledTime: number
  noRetry(): void
}

interface ExportedHandler<Env = unknown> {
  fetch?: (request: Request, env: Env, ctx: ExecutionContext) => Response | Promise<Response>
  scheduled?: (controller: ScheduledController, env: Env, ctx: ExecutionContext) => void | Promise<void>
}

interface KVNamespace {
  delete(key: string): Promise<void>
  get<T = string>(key: string, options?: unknown): Promise<T | null>
  list<T = unknown>(options?: unknown): Promise<T>
  put(key: string, value: string | ArrayBuffer | ArrayBufferView | ReadableStream, options?: unknown): Promise<void>
}

interface Queue<T = unknown> {
  send(message: T, options?: unknown): Promise<void>
  sendBatch(messages: ReadonlyArray<Readonly<{ body: T }>>): Promise<void>
}

interface MessageBatch<T = unknown> {
  readonly messages: ReadonlyArray<Readonly<{ body: T; ack(): void; retry(options?: unknown): void }>>
  readonly queue: string
  ackAll(): void
  retryAll(options?: unknown): void
}

interface EmailMessage {
  readonly from: string
  readonly raw: ReadableStream
  readonly rawSize: number
  readonly to: string
  forward(rcptTo: string, headers?: Headers): Promise<void>
  reply(message: EmailMessage): Promise<void>
  setReject(reason: string): void
}

interface SendEmail {
  send(message: EmailMessage): Promise<void>
}

declare class ForwardableEmailMessage implements EmailMessage {
  constructor(from: string, to: string, raw: string | ReadableStream)
  readonly from: string
  readonly raw: ReadableStream
  readonly rawSize: number
  readonly to: string
  forward(rcptTo: string, headers?: Headers): Promise<void>
  reply(message: EmailMessage): Promise<void>
  setReject(reason: string): void
}
