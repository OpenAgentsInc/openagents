declare module "@radix-ui/react-use-callback-ref" {
  import type { RefCallback } from "react";
  // Minimal stub that returns a stable callback ref
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  export function useCallbackRef<T extends (...args: any[]) => any>(fn: T): T;
}

