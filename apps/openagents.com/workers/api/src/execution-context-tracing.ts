type NoopSpan = Readonly<{
  end: () => void
  isTraced: false
  setAttribute: (key: string, value?: boolean | number | string) => void
}>

const noopSpan: NoopSpan = {
  end: () => undefined,
  isTraced: false,
  setAttribute: () => undefined,
}

export type OpenAgentsRequestTracing = Readonly<{
  enterSpan: <Args extends ReadonlyArray<unknown>, Result>(
    name: string,
    callback: (span: NoopSpan, ...args: Args) => Result,
    ...args: Args
  ) => Result
  startActiveSpan: <Args extends ReadonlyArray<unknown>, Result>(
    name: string,
    callback: (span: NoopSpan, ...args: Args) => Result,
    ...args: Args
  ) => Result
}>

export const noopExecutionContextTracing: OpenAgentsRequestTracing = {
  enterSpan: (_name, callback, ...args) => callback(noopSpan, ...args),
  startActiveSpan: (_name, callback, ...args) => callback(noopSpan, ...args),
}
