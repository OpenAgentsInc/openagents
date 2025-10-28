// Install minimal global error handlers to surface exact errors in production
// This file has side effects when imported.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
declare const ErrorUtils: any;

try {
  const defaultHandler = typeof (global as any).ErrorUtils?.getGlobalHandler === 'function'
    ? (global as any).ErrorUtils.getGlobalHandler()
    : null

  if (typeof (global as any).ErrorUtils?.setGlobalHandler === 'function') {
    (global as any).ErrorUtils.setGlobalHandler((err: any, isFatal?: boolean) => {
      try { console.log('[global.error]', isFatal ? 'FATAL' : 'non-fatal', String(err?.message || err)); } catch {}
      try { if (err?.stack) console.log('[global.error.stack]', String(err.stack)); } catch {}
      if (defaultHandler) {
        try { defaultHandler(err, isFatal) } catch {}
      }
    })
  }
} catch {}

