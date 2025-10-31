// Install minimal global error handlers to surface exact errors in production
// This file has side effects when imported.
declare const ErrorUtils: { getGlobalHandler?: () => ((error: unknown, isFatal?: boolean) => void) | undefined; setGlobalHandler?: (fn: (error: unknown, isFatal?: boolean) => void) => void };

try {
  const defaultHandler = typeof (global as any).ErrorUtils?.getGlobalHandler === 'function'
    ? (global as any).ErrorUtils.getGlobalHandler()
    : null

  if (typeof (global as any).ErrorUtils?.setGlobalHandler === 'function') {
    (global as any).ErrorUtils.setGlobalHandler((err: unknown, isFatal?: boolean) => {
      try { console.log('[global.error]', isFatal ? 'FATAL' : 'non-fatal', String((err as Error)?.message || err)); } catch {}
      try { if ((err as Error)?.stack) console.log('[global.error.stack]', String((err as Error).stack)); } catch {}
      if (defaultHandler) {
        try { defaultHandler(err, isFatal) } catch {}
      }
    })
  }
} catch {}
