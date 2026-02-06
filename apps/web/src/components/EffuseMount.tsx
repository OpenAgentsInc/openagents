import { Effect } from "effect";
import { useEffect, useRef } from "react";

type RunEffuse = (container: Element) => Effect.Effect<void>;

interface EffuseMountProps {
  run: RunEffuse;
  deps?: ReadonlyArray<unknown>;
  className?: string;
}

/**
 * Renders a div and runs the given Effuse program to fill it (client-side).
 */
export function EffuseMount({ run, deps = [], className }: EffuseMountProps) {
  const ref = useRef<HTMLDivElement>(null);
  const runRef = useRef(run);
  runRef.current = run;

  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    const program = runRef.current(el);
    let cancelled = false;

    Effect.runPromise(program).catch((err) => {
      if (!cancelled) console.error("[EffuseMount]", err);
    });

    return () => {
      cancelled = true;
    };
  }, deps);

  return <div ref={ref} className={className} />;
}
