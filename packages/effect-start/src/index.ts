import { AsyncLocalStorage } from 'node:async_hooks'
import { Effect, Schema as S } from 'effect'

export type StartExecutionContext = Readonly<{
  waitUntil(promise: Promise<unknown>): void
}>

export type StartRequestContext<Env = unknown> = Readonly<{
  request: Request
  env: Env
  executionCtx?: StartExecutionContext
}>

export class StartRequestContextMissing extends S.TaggedErrorClass<StartRequestContextMissing>()(
  'StartRequestContextMissing',
  {
    operation: S.String,
    reasonRef: S.Literal('start.request_context.missing'),
  },
) {}

export class StartInputDecodeError extends S.TaggedErrorClass<StartInputDecodeError>()(
  'StartInputDecodeError',
  {
    operation: S.String,
    reasonRef: S.Literal('start.input.schema_invalid'),
  },
) {}

export class StartHttpError extends S.TaggedErrorClass<StartHttpError>()(
  'StartHttpError',
  {
    status: S.Number,
    reasonRef: S.String,
    messageSafe: S.String,
  },
) {}

export type StartBridgeError =
  | StartRequestContextMissing
  | StartInputDecodeError
  | StartHttpError

export type StartEffectProvider<R> = <A, E>(
  effect: Effect.Effect<A, E, R>,
) => Effect.Effect<A, E>

export type StartJsonHandlerOptions<Env, Input, A, E, R> = Readonly<{
  context: StartRequestContext<Env>
  input: unknown
  inputSchema: S.Decoder<Input>
  operation: string
  effect(input: Input): Effect.Effect<A, E, R>
  mapError?: (error: E | StartBridgeError) => Response
  mapSuccess?: (value: A) => Response
}>

export type EffectStartRuntime<R> = Readonly<{
  run<A, E>(effect: Effect.Effect<A, E, R>): Promise<A>
  runWithContext<Env, A, E>(
    context: StartRequestContext<Env>,
    effect: Effect.Effect<A, E, R>,
  ): Promise<A>
  handleJson<Env, Input, A, E>(
    options: StartJsonHandlerOptions<Env, Input, A, E, R>,
  ): Promise<Response>
}>

const requestContextStorage =
  new AsyncLocalStorage<StartRequestContext<unknown>>()

export const withStartRequestContext = <Env, A>(
  context: StartRequestContext<Env>,
  fn: () => A,
): A => requestContextStorage.run(context as StartRequestContext<unknown>, fn)

export const getStartRequestContext = <
  Env = unknown,
>(): StartRequestContext<Env> | undefined =>
  requestContextStorage.getStore() as StartRequestContext<Env> | undefined

export const currentStartRequestContext = <Env = unknown>(
  operation = 'start.request_context.current',
): Effect.Effect<StartRequestContext<Env>, StartRequestContextMissing> =>
  Effect.suspend(() => {
    const context = getStartRequestContext<Env>()
    return context
      ? Effect.succeed(context)
      : Effect.fail(
          new StartRequestContextMissing({
            operation,
            reasonRef: 'start.request_context.missing',
          }),
        )
  })

export const currentStartRequest = (
  operation = 'start.request.current',
): Effect.Effect<Request, StartRequestContextMissing> =>
  currentStartRequestContext(operation).pipe(
    Effect.map(context => context.request),
  )

export const currentStartEnv = <Env = unknown>(
  operation = 'start.env.current',
): Effect.Effect<Env, StartRequestContextMissing> =>
  currentStartRequestContext<Env>(operation).pipe(
    Effect.map(context => context.env),
  )

export const currentStartExecutionContext = (
  operation = 'start.execution_context.current',
): Effect.Effect<
  StartExecutionContext,
  StartRequestContextMissing | StartHttpError
> =>
  currentStartRequestContext(operation).pipe(
    Effect.flatMap(context =>
      context.executionCtx
        ? Effect.succeed(context.executionCtx)
        : Effect.fail(
            new StartHttpError({
              status: 500,
              reasonRef: 'start.execution_context.missing',
              messageSafe: 'Start execution context is unavailable',
            }),
          ),
    ),
  )

export const decodeStartInput = <A>(
  schema: S.Decoder<A>,
  input: unknown,
  operation: string,
): Effect.Effect<A, StartInputDecodeError> =>
  S.decodeUnknownEffect(schema)(input).pipe(
    Effect.mapError(
      () =>
        new StartInputDecodeError({
          operation,
          reasonRef: 'start.input.schema_invalid',
        }),
    ),
  )

export const jsonResponse = (
  body: unknown,
  init: ResponseInit = {},
): Response => {
  const headers = new Headers(init.headers)

  if (!headers.has('Content-Type')) {
    headers.set('Content-Type', 'application/json; charset=utf-8')
  }

  return new Response(JSON.stringify(body), {
    ...init,
    headers,
  })
}

export const startErrorToResponse = (error: unknown): Response => {
  if (error instanceof StartInputDecodeError) {
    return jsonResponse(
      {
        error: error.reasonRef,
        operation: error.operation,
      },
      { status: 400 },
    )
  }

  if (error instanceof StartRequestContextMissing) {
    return jsonResponse(
      {
        error: error.reasonRef,
        operation: error.operation,
      },
      { status: 500 },
    )
  }

  if (error instanceof StartHttpError) {
    return jsonResponse(
      {
        error: error.reasonRef,
        message: error.messageSafe,
      },
      { status: error.status },
    )
  }

  return jsonResponse(
    {
      error: 'start.unexpected',
      message: 'Unexpected Start handler failure',
    },
    { status: 500 },
  )
}

export const makeEffectStartRuntime = <R>(
  provideRuntime: StartEffectProvider<R>,
): EffectStartRuntime<R> => {
  const run = <A, E>(effect: Effect.Effect<A, E, R>): Promise<A> =>
    Effect.runPromise(provideRuntime(effect))

  const runWithContext = <Env, A, E>(
    context: StartRequestContext<Env>,
    effect: Effect.Effect<A, E, R>,
  ): Promise<A> => withStartRequestContext(context, () => run(effect))

  const handleJson = <Env, Input, A, E>(
    options: StartJsonHandlerOptions<Env, Input, A, E, R>,
  ): Promise<Response> => {
    const responseEffect = decodeStartInput(
      options.inputSchema,
      options.input,
      options.operation,
    ).pipe(
      Effect.flatMap(input => options.effect(input)),
      Effect.match({
        onFailure: error =>
          options.mapError
            ? options.mapError(error)
            : startErrorToResponse(error),
        onSuccess: value =>
          options.mapSuccess ? options.mapSuccess(value) : jsonResponse(value),
      }),
    )

    return runWithContext(options.context, responseEffect)
  }

  return {
    handleJson,
    run,
    runWithContext,
  }
}

export const effectStartRuntime = makeEffectStartRuntime<never>(effect => effect)

export const scheduleStartTask = (
  createTask: () => Promise<unknown>,
): Effect.Effect<boolean, StartRequestContextMissing> =>
  currentStartRequestContext('start.execution_context.schedule').pipe(
    Effect.map(context => {
      if (!context.executionCtx) {
        return false
      }

      context.executionCtx.waitUntil(createTask())
      return true
    }),
  )
