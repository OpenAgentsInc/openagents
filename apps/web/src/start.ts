import { createStart, createMiddleware } from '@tanstack/react-start';
import { authkitMiddleware } from '@workos/authkit-tanstack-react-start';

export const startInstance = createStart(() => {
  const authkit = authkitMiddleware();
  const safeAuthkit = createMiddleware({ type: 'request' }).server(async (opts) => {
    try {
      if (authkit.options?.server) {
        return await (authkit.options.server as (o: typeof opts) => Promise<unknown>)(opts);
      }
      return opts.next();
    } catch (err) {
      console.error('AuthKit middleware error (continuing unauthenticated):', err);
      return opts.next();
    }
  });
  return {
    requestMiddleware: [safeAuthkit],
  };
});
