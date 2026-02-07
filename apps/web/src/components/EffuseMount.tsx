import { useRouter } from '@tanstack/react-router';
import { Effect } from 'effect';
import { useEffect, useMemo, useRef } from 'react';
import { EffuseLive, mountEzRuntimeWith } from '@openagentsinc/effuse';
import type { EzAction } from '@openagentsinc/effuse';

type RunEffuse = (container: Element) => Effect.Effect<void>;

interface EffuseMountProps {
  run: RunEffuse;
  deps?: ReadonlyArray<unknown>;
  className?: string;
  /**
   * Optional server-rendered HTML for this mount container.
   *
   * When provided, EffuseMount will:
   * - render this HTML via `dangerouslySetInnerHTML` (SSR + initial hydration)
   * - skip the initial `run()` call to avoid tearing down the server DOM
   *
   * Keep this value stable for the lifetime of the mount to prevent React from
   * mutating `innerHTML` after hydration.
   */
  ssrHtml?: string;
  /**
   * Optional client-side hydration program to run when `ssrHtml` is present.
   * Use this to attach behaviors that can't run on the server (e.g. canvases,
   * observers) without rerendering/replacing DOM.
   */
  hydrate?: RunEffuse;
  /** Called after the Effuse program has rendered (e.g. to attach event delegation). */
  onRendered?: (container: Element) => void;
  /** Called when the mount is about to be re-rendered or unmounted (e.g. to dispose observers). */
  onCleanup?: (container: Element) => void;
  /**
   * Control when `onCleanup()` is invoked:
   * - `both` (default): called on rerender + unmount
   * - `rerender`: called only when the mount is about to rerender
   * - `unmount`: called only when the mount is unmounting
   */
  cleanupOn?: 'both' | 'rerender' | 'unmount';
  /**
   * Optional Effuse `data-ez` registry. When provided, EffuseMount installs
   * the delegated event runtime once for this mount container.
   *
   * Important: keep the Map identity stable (mutate via `.set(...)` if you need
   * to update handlers over time).
   */
  ezRegistry?: Map<string, EzAction>;
}

/**
 * Renders a div and runs the given Effuse program to fill it (client-side).
 */
export function EffuseMount({
  run,
  deps = [],
  className,
  ssrHtml,
  hydrate,
  onRendered,
  onCleanup,
  cleanupOn = 'both',
  ezRegistry,
}: EffuseMountProps) {
  const router = useRouter();
  const ref = useRef<HTMLDivElement>(null);
  const runRef = useRef(run);
  const hydrateRef = useRef(hydrate);
  const onRenderedRef = useRef(onRendered);
  const onCleanupRef = useRef(onCleanup);
  const ezRegistryRef = useRef<EffuseMountProps['ezRegistry']>(undefined);
  const ezMountedRef = useRef(false);
  const didSkipInitialRenderRef = useRef(false);
  const isUnmountingRef = useRef(false);
  runRef.current = run;
  hydrateRef.current = hydrate;
  onRenderedRef.current = onRendered;
  onCleanupRef.current = onCleanup;
  ezRegistryRef.current = ezRegistry;

  // Stable object for dangerouslySetInnerHTML so React does not re-apply on parent re-renders.
  // Must be unconditional (hooks rules).
  const stableInnerHTML = useMemo(
    () => (ssrHtml !== undefined ? { __html: ssrHtml } : { __html: '' }),
    [ssrHtml],
  );

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

  // Mount the `data-ez` runtime once per mount container.
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    if (ezMountedRef.current) return;

    const registry = ezRegistryRef.current;
    if (!registry) return;

    ezMountedRef.current = true;
    Effect.runPromise(mountEzRuntimeWith(el, registry).pipe(Effect.provide(EffuseLive))).catch((err) => {
      console.error('[EffuseMount/Ez]', err);
    });
  }, [ezRegistry]);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    // If we have SSR HTML, the DOM is already present. Avoid calling `run()`
    // on the first client effect, since `DomService.render` would replace
    // the subtree and cause flicker.
    if (ssrHtml !== undefined && !didSkipInitialRenderRef.current) {
      didSkipInitialRenderRef.current = true;

      const hydrateProgram = hydrateRef.current?.(el) ?? Effect.void;
      let cancelled = false;

      Effect.runPromise(hydrateProgram)
        .then(() => {
          if (!cancelled) onRenderedRef.current?.(el);
        })
        .catch((err) => {
          if (!cancelled) console.error('[EffuseMount/hydrate]', err);
        });

      return () => {
        cancelled = true;
        const isUnmounting = isUnmountingRef.current;
        const shouldCleanup =
          cleanupOn === 'both'
            ? true
            : cleanupOn === 'unmount'
              ? isUnmounting
              : !isUnmounting;
        if (shouldCleanup) onCleanupRef.current?.(el);
      };
    }

    const program = runRef.current(el);
    let cancelled = false;

    Effect.runPromise(program)
      .then(() => {
        if (!cancelled) onRenderedRef.current?.(el);
      })
      .catch((err) => {
        if (!cancelled) console.error('[EffuseMount]', err);
      });

    return () => {
      cancelled = true;
      const isUnmounting = isUnmountingRef.current;
      const shouldCleanup =
        cleanupOn === 'both' ? true : cleanupOn === 'unmount' ? isUnmounting : !isUnmounting;
      if (shouldCleanup) onCleanupRef.current?.(el);
    };
  }, deps);

  // Set an unmount marker so effect cleanups can tell rerender vs unmount.
  // Declared after the render effect so its cleanup runs first on unmount.
  useEffect(() => {
    return () => {
      isUnmountingRef.current = true;
    };
  }, []);

  if (ssrHtml !== undefined) {
    return (
      <div
        ref={ref}
        className={className}
        dangerouslySetInnerHTML={stableInnerHTML}
      />
    );
  }

  return <div ref={ref} className={className} />;
}
