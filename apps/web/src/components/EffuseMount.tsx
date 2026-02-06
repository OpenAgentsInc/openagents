import { useRouter } from '@tanstack/react-router';
import { Effect } from 'effect';
import { useEffect, useRef } from 'react';

type RunEffuse = (container: Element) => Effect.Effect<void>;

interface EffuseMountProps {
  run: RunEffuse;
  deps?: ReadonlyArray<unknown>;
  className?: string;
  /** Called after the Effuse program has rendered (e.g. to attach event delegation). */
  onRendered?: () => void;
}

/**
 * Renders a div and runs the given Effuse program to fill it (client-side).
 */
export function EffuseMount({ run, deps = [], className, onRendered }: EffuseMountProps) {
  const router = useRouter();
  const ref = useRef<HTMLDivElement>(null);
  const runRef = useRef(run);
  const onRenderedRef = useRef(onRendered);
  runRef.current = run;
  onRenderedRef.current = onRendered;

  // Intercept internal <a href="/..."> links inside Effuse-rendered DOM so we navigate
  // via TanStack Router (SPA) instead of full page reloads.
  useEffect(() => {
    const container = ref.current;
    if (!container) return;

    const onClick = (e: MouseEvent) => {
      if (e.defaultPrevented) return;
      if (e.button !== 0) return;
      if (e.metaKey || e.altKey || e.ctrlKey || e.shiftKey) return;

      const target = e.target;
      if (!(target instanceof Element)) return;

      const anchor = target.closest('a[href]');
      if (!(anchor instanceof HTMLAnchorElement)) return;

      // Let the browser handle non-default behaviors.
      if (anchor.hasAttribute('data-router-ignore')) return;
      const hrefAttr = anchor.getAttribute('href');
      if (!hrefAttr) return;
      if (hrefAttr.startsWith('#')) return;
      if (hrefAttr.startsWith('mailto:') || hrefAttr.startsWith('tel:')) return;
      if (anchor.hasAttribute('download')) return;
      if (anchor.target && anchor.target !== '_self') return;

      let url: URL;
      try {
        url = new URL(hrefAttr, window.location.href);
      } catch {
        return;
      }

      // External navigation should remain a full document navigation.
      if (url.origin !== window.location.origin) return;

      // Let the router handle internal navigation (including search/hash).
      e.preventDefault();
      const href = `${url.pathname}${url.search}${url.hash}`;
      router.navigate({ href }).catch(() => {});
    };

    container.addEventListener('click', onClick);
    return () => container.removeEventListener('click', onClick);
  }, [router]);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    const program = runRef.current(el);
    let cancelled = false;

    Effect.runPromise(program)
      .then(() => {
        if (!cancelled) onRenderedRef.current?.();
      })
      .catch((err) => {
        if (!cancelled) console.error('[EffuseMount]', err);
      });

    return () => {
      cancelled = true;
    };
  }, deps);

  return <div ref={ref} className={className} />;
}
