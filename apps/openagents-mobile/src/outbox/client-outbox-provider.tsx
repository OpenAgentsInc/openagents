import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from "react";

import {
  openMobileClientOutboxRuntime,
  type MobileClientOutboxRuntime,
} from "./client-outbox-runtime";

export type MobileClientOutboxState =
  | Readonly<{ phase: "initializing"; runtime: null; error: null }>
  | Readonly<{ phase: "ready"; runtime: MobileClientOutboxRuntime; error: null }>
  | Readonly<{ phase: "failed"; runtime: null; error: Error }>;

const MobileClientOutboxContext = createContext<MobileClientOutboxState>({
  phase: "initializing",
  runtime: null,
  error: null,
});

export const useMobileClientOutbox = (): MobileClientOutboxState =>
  useContext(MobileClientOutboxContext);

export const MobileClientOutboxProvider = ({ children }: { readonly children: ReactNode }) => {
  const [state, setState] = useState<MobileClientOutboxState>({
    phase: "initializing",
    runtime: null,
    error: null,
  });

  useEffect(() => {
    let current: MobileClientOutboxRuntime | null = null;
    let cancelled = false;
    void openMobileClientOutboxRuntime()
      .then((runtime) => {
        if (cancelled) {
          void runtime.close();
          return;
        }
        current = runtime;
        setState({ phase: "ready", runtime, error: null });
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setState({
            phase: "failed",
            runtime: null,
            error:
              error instanceof Error
                ? error
                : new Error("The mobile command outbox failed to initialize."),
          });
        }
      });
    return () => {
      cancelled = true;
      if (current !== null) void current.close();
    };
  }, []);

  const value = useMemo(() => state, [state]);
  return (
    <MobileClientOutboxContext.Provider value={value}>
      {children}
    </MobileClientOutboxContext.Provider>
  );
};
