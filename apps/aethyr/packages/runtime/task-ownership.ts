/** Trusted runtime scheduling checks; borrowed JavaScript values are not a security boundary. */
type ResourceKey<Resources extends object> = Extract<keyof Resources, string>;
type SyncResult<Result> = Result extends PromiseLike<unknown> ? never : unknown;
export interface ResourceHandle {
  readonly id: string;
  readonly owner: string;
  readonly generation: number;
}
export interface ResourceBorrow<Value> extends ResourceHandle {
  valid(): boolean;
  /** Synchronous borrow. Do not retain its raw value outside this callback. */
  withView<Result>(body: (value: Value) => Result & SyncResult<Result>): Result;
}
export interface TaskDefinition<Key extends string = string> {
  readonly id: string;
  readonly owner: string;
  readonly reads?: readonly Key[];
  readonly writes?: readonly Key[];
}
export interface TaskContext<Resources extends object> {
  readonly taskId: string;
  read<Key extends ResourceKey<Resources>>(
    id: Key,
  ): ResourceBorrow<Readonly<Resources[Key]>>;
  write<Key extends ResourceKey<Resources>>(
    id: Key,
  ): ResourceBorrow<Resources[Key]>;
}
export interface TaskGroup {
  readonly tasks: readonly string[];
}
export interface OwnershipSnapshot {
  resources: number;
  tasks: number;
  activeTasks: number;
  generation: number;
  runs: number;
  borrows: number;
  safePoints: number;
  conflicts: number;
  accessErrors: number;
  cancellations: number;
  closed: boolean;
}
interface Entry {
  id: string;
  owner: string;
  generation: number;
  value: unknown;
  disposed: boolean;
}
interface Declaration {
  id: string;
  owner: string;
  reads: Set<string>;
  writes: Set<string>;
}
interface Invocation {
  serial: number;
  declaration: Declaration;
  active: boolean;
  canceled: boolean;
}
const MAX_RESOURCES = 256,
  MAX_TASKS = 256,
  MAX_ACTIVE = 64;
const identifier = (value: unknown): value is string =>
  typeof value === "string" && /^[a-zA-Z0-9_.:-]{1,96}$/.test(value);
function thenable(value: unknown): value is PromiseLike<unknown> {
  return (
    value !== null &&
    (typeof value === "object" || typeof value === "function") &&
    typeof (value as { then?: unknown }).then === "function"
  );
}
function requireSyncFunction(
  body: unknown,
): asserts body is (...args: any[]) => unknown {
  if (typeof body !== "function" || body.constructor?.name === "AsyncFunction")
    throw new Error(
      "Runtime tasks and borrowed views need synchronous callbacks.",
    );
}
function requireSyncResult(value: unknown) {
  if (thenable(value)) {
    // A rejected accidental async callback must not become an unhandled rejection.
    void Promise.resolve(value).catch(() => {});
    throw new Error(
      "Finish the runtime task before returning asynchronous work.",
    );
  }
}
function conflicts(a: Declaration, b: Declaration): boolean {
  for (const key of a.writes)
    if (b.reads.has(key) || b.writes.has(key)) return true;
  for (const key of b.writes) if (a.reads.has(key)) return true;
  return false;
}

/** Declared access and synchronous lifetimes around the existing owned runtime loop. */
export class TaskOwnership<Resources extends object = Record<string, unknown>> {
  readonly #resources = new Map<string, Entry>();
  readonly #tasks = new Map<string, Declaration>();
  readonly #active = new Map<number, Invocation>();
  #generation = 0;
  #resourceGeneration = 0;
  #serial = 0;
  #safeDepth = 0;
  #started = false;
  #closed = false;
  #counters = {
    runs: 0,
    borrows: 0,
    safePoints: 0,
    conflicts: 0,
    accessErrors: 0,
    cancellations: 0,
  };

  /** Startup registration is open until the first task runs; later changes use a safe point. */
  resource<Key extends ResourceKey<Resources>>(
    id: Key,
    value: Resources[Key],
    owner: string,
  ): ResourceHandle {
    this.#structural();
    if (
      !identifier(id) ||
      !identifier(owner) ||
      this.#resources.has(id) ||
      this.#resources.size >= MAX_RESOURCES
    )
      throw new Error(
        "Register a distinct, bounded runtime resource and its owner.",
      );
    const generation = this.#nextGeneration();
    const entry: Entry = { id, owner, generation, value, disposed: false };
    this.#resources.set(id, entry);
    this.#generation++;
    return this.#handle(entry);
  }
  define(definition: TaskDefinition<ResourceKey<Resources>>): void {
    this.#structural();
    if (
      !definition ||
      !identifier(definition.id) ||
      !identifier(definition.owner) ||
      this.#tasks.has(definition.id) ||
      this.#tasks.size >= MAX_TASKS
    )
      throw new Error("Define a distinct, bounded runtime task and its owner.");
    const reads = definition.reads ?? [],
      writes = definition.writes ?? [];
    if (
      !Array.isArray(reads) ||
      !Array.isArray(writes) ||
      reads.length > MAX_RESOURCES ||
      writes.length > MAX_RESOURCES ||
      new Set(reads).size !== reads.length ||
      new Set(writes).size !== writes.length ||
      reads.some((key) => writes.includes(key))
    )
      throw new Error(
        "Declare each task resource once; a write already permits reading.",
      );
    const declaration = {
      id: definition.id,
      owner: definition.owner,
      reads: new Set<string>(reads),
      writes: new Set<string>(writes),
    };
    this.#validateDeclaration(declaration);
    this.#tasks.set(declaration.id, declaration);
    this.#generation++;
  }
  /** Reject a proposed parallel group before starting any task in it. */
  validateGroup(taskIds: readonly string[]): void {
    const declarations = this.#declarations(taskIds);
    for (let a = 0; a < declarations.length; a++)
      for (let b = a + 1; b < declarations.length; b++)
        if (conflicts(declarations[a], declarations[b])) {
          this.#counters.conflicts++;
          throw new Error(
            `Tasks ${declarations[a].id} and ${declarations[b].id} have conflicting resource access.`,
          );
        }
  }
  /** Preserve the input order of every conflicting pair while grouping independent tasks. */
  plan(taskIds: readonly string[]): readonly TaskGroup[] {
    const declarations = this.#declarations(taskIds),
      levels: number[] = [],
      groups: string[][] = [];
    declarations.forEach((task, i) => {
      let level = 0;
      for (let prior = 0; prior < i; prior++)
        if (conflicts(task, declarations[prior]))
          level = Math.max(level, levels[prior] + 1);
      levels.push(level);
      (groups[level] ??= []).push(task.id);
    });
    return Object.freeze(
      groups.map((tasks) => Object.freeze({ tasks: Object.freeze(tasks) })),
    );
  }
  run<Result>(
    taskId: string,
    body: (context: TaskContext<Resources>) => Result & SyncResult<Result>,
  ): Result {
    this.#open();
    requireSyncFunction(body);
    const declaration = this.#declaration(taskId);
    if (this.#active.size >= MAX_ACTIVE)
      throw new Error(
        "Finish the current runtime tasks before nesting more work.",
      );
    for (const running of this.#active.values())
      if (
        running.declaration.id === taskId ||
        conflicts(running.declaration, declaration)
      ) {
        this.#counters.conflicts++;
        throw new Error(
          "Finish conflicting runtime work before starting this task.",
        );
      }
    this.#started = true;
    const invocation = {
      serial: ++this.#serial,
      declaration,
      active: true,
      canceled: false,
    };
    this.#active.set(invocation.serial, invocation);
    this.#counters.runs++;
    const context: TaskContext<Resources> = Object.freeze({
      taskId,
      read: <Key extends ResourceKey<Resources>>(id: Key) =>
        this.#borrow<Readonly<Resources[Key]>>(invocation, id, false),
      write: <Key extends ResourceKey<Resources>>(id: Key) =>
        this.#borrow<Resources[Key]>(invocation, id, true),
    });
    try {
      const result = body(context);
      requireSyncResult(result);
      this.#invocationValid(invocation);
      return result;
    } finally {
      invocation.active = false;
      this.#active.delete(invocation.serial);
    }
  }
  runSerial<Result>(
    taskIds: readonly string[],
    body: (
      taskId: string,
      context: TaskContext<Resources>,
    ) => Result & SyncResult<Result>,
  ): Result[] {
    this.#declarations(taskIds);
    requireSyncFunction(body);
    return taskIds.map((id) => this.run(id, (context) => body(id, context)));
  }
  safePoint<Result>(body: () => Result & SyncResult<Result>): Result {
    this.#open();
    requireSyncFunction(body);
    if (this.#active.size || this.#safeDepth)
      throw new Error(
        "Finish runtime tasks before entering a structural safe point.",
      );
    this.#safeDepth++;
    this.#counters.safePoints++;
    try {
      const result = body();
      requireSyncResult(result);
      return result;
    } finally {
      this.#safeDepth--;
    }
  }
  replace<Key extends ResourceKey<Resources>>(
    id: Key,
    value: Resources[Key],
    owner?: string,
  ): ResourceHandle {
    this.#requireSafePoint();
    const previous = this.#entry(id);
    if (owner !== undefined && !identifier(owner))
      throw new Error("Choose a valid runtime resource owner.");
    const generation = this.#nextGeneration();
    const next = {
      id,
      value,
      owner: owner ?? previous.owner,
      generation,
      disposed: false,
    };
    previous.disposed = true;
    this.#resources.set(id, next);
    this.#generation++;
    return this.#handle(next);
  }
  disposeResource<Key extends ResourceKey<Resources>>(id: Key): void {
    this.#requireSafePoint();
    const entry = this.#entry(id);
    const generation = this.#nextGeneration();
    entry.disposed = true;
    entry.generation = generation;
    this.#resources.delete(id);
    this.#generation++;
  }
  valid(handle: ResourceHandle): boolean {
    const entry = handle && this.#resources.get(handle.id);
    return (
      !this.#closed &&
      !!entry &&
      !entry.disposed &&
      entry.owner === handle.owner &&
      entry.generation === handle.generation
    );
  }
  /** Cooperative cancellation invalidates borrows; it does not preempt arbitrary JavaScript. */
  cancel(taskId?: string): number {
    let canceled = 0;
    for (const invocation of this.#active.values())
      if (
        (!taskId || taskId === invocation.declaration.id) &&
        !invocation.canceled
      ) {
        invocation.canceled = true;
        canceled++;
      }
    this.#counters.cancellations += canceled;
    return canceled;
  }
  close(): void {
    if (this.#closed) return;
    this.cancel();
    this.#closed = true;
    for (const entry of this.#resources.values()) entry.disposed = true;
    for (const invocation of this.#active.values()) invocation.active = false;
    this.#active.clear();
    this.#resources.clear();
    this.#tasks.clear();
    this.#generation++;
  }
  snapshot(): OwnershipSnapshot {
    return {
      resources: this.#resources.size,
      tasks: this.#tasks.size,
      activeTasks: this.#active.size,
      generation: this.#generation,
      ...this.#counters,
      closed: this.#closed,
    };
  }
  #borrow<Value>(
    invocation: Invocation,
    id: string,
    write: boolean,
  ): ResourceBorrow<Value> {
    this.#invocationValid(invocation);
    const declaration = invocation.declaration;
    if (
      write
        ? !declaration.writes.has(id)
        : !declaration.reads.has(id) && !declaration.writes.has(id)
    ) {
      this.#counters.accessErrors++;
      throw new Error(
        `Task ${declaration.id} did not declare ${write ? "write" : "read"} access to ${id}.`,
      );
    }
    const entry = this.#entry(id),
      generation = entry.generation;
    if (write && entry.owner !== declaration.owner) {
      this.#counters.accessErrors++;
      throw new Error("Only the resource owner may write its value.");
    }
    this.#counters.borrows++;
    const valid = () =>
      !this.#closed &&
      invocation.active &&
      !invocation.canceled &&
      !entry.disposed &&
      this.#resources.get(id) === entry &&
      entry.generation === generation;
    return Object.freeze({
      id,
      owner: entry.owner,
      generation,
      valid,
      withView: <Result>(
        body: (value: Value) => Result & SyncResult<Result>,
      ): Result => {
        if (!valid())
          throw new Error(
            "This runtime borrow expired or its resource changed.",
          );
        requireSyncFunction(body);
        const result = body(entry.value as Value);
        requireSyncResult(result);
        if (!valid())
          throw new Error(
            "This runtime borrow expired or its task was canceled.",
          );
        return result;
      },
    });
  }
  #open() {
    if (this.#closed)
      throw new Error("Open a new ownership scope after closing this one.");
  }
  #structural() {
    this.#open();
    if (this.#active.size || (this.#started && !this.#safeDepth))
      throw new Error(
        "Change runtime structure only at a safe point between tasks.",
      );
  }
  #requireSafePoint() {
    this.#open();
    if (!this.#safeDepth || this.#active.size)
      throw new Error(
        "Replace or dispose runtime resources at a structural safe point.",
      );
  }
  #nextGeneration() {
    const next = this.#resourceGeneration + 1;
    if (!Number.isSafeInteger(next))
      throw new Error("This runtime resource exhausted its generations.");
    this.#resourceGeneration = next;
    return next;
  }
  #entry(id: string) {
    const entry = this.#resources.get(id);
    if (!entry || entry.disposed) {
      this.#counters.accessErrors++;
      throw new Error(`Runtime resource ${id} is not registered.`);
    }
    return entry;
  }
  #validateDeclaration(task: Declaration) {
    for (const id of [...task.reads, ...task.writes])
      if (!identifier(id)) throw new Error("Choose valid task resource IDs.");
    for (const id of task.reads) this.#entry(id);
    for (const id of task.writes)
      if (this.#entry(id).owner !== task.owner) {
        this.#counters.accessErrors++;
        throw new Error(
          `Task ${task.id} cannot write a resource owned by another runtime owner.`,
        );
      }
  }
  #declaration(id: string) {
    const task = this.#tasks.get(id);
    if (!task) throw new Error(`Runtime task ${id} has no access declaration.`);
    this.#validateDeclaration(task);
    return task;
  }
  #declarations(ids: readonly string[]) {
    this.#open();
    if (
      !Array.isArray(ids) ||
      ids.length > MAX_TASKS ||
      new Set(ids).size !== ids.length
    )
      throw new Error("Plan a bounded list of distinct runtime tasks.");
    return ids.map((id) => this.#declaration(id));
  }
  #invocationValid(invocation: Invocation) {
    this.#open();
    if (!invocation.active || invocation.canceled)
      throw new Error("This runtime task ended or was canceled.");
  }
  #handle(entry: Entry): ResourceHandle {
    return Object.freeze({
      id: entry.id,
      owner: entry.owner,
      generation: entry.generation,
    });
  }
}
