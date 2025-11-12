"use client";

import { useEffect, useMemo, useState } from "react";
import { ExternalStoreRuntimeCore } from "../legacy-runtime/runtime-cores/external-store/ExternalStoreRuntimeCore";
import type { ExternalStoreAdapter } from "../legacy-runtime/runtime-cores/external-store/ExternalStoreAdapter";
import { AssistantRuntimeImpl } from "../legacy-runtime/runtime/AssistantRuntime";

// Minimal hook: does not wire modelContext providers; focuses on message repo plumbing
export const useExternalStoreRuntime = <T,>(
  store: ExternalStoreAdapter<T>,
) => {
  const [runtime] = useState(() => new ExternalStoreRuntimeCore(store));

  useEffect(() => {
    runtime.setAdapter(store);
  }, [runtime, store]);

  return useMemo(() => new AssistantRuntimeImpl(runtime as any), [runtime]);
};

