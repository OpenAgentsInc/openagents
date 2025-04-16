// This is a client-safe version of the auth utilities
// It primarily exists to maintain type compatibility and imports across the codebase

// Define the types that will be used by client components
export interface AuthUser {
  id: string;
  name: string;
  email?: string;
  image?: string | null;
}

export interface AuthSession {
  id: string;
  userId: string;
  expires: string;
}

// The auth object is not actually used on the client side
// It's just a placeholder to maintain consistent imports
export const auth = {
  api: {
    getSession: async () => {
      console.warn('Auth is not available on the client side');
      return null;
    }
  }
};

// Client-side version of requireAuth that simply indicates redirection
// This is never actually used on the client side, but exists for type compatibility
export async function requireAuth(request: Request) {
  console.warn('Auth is not available on the client side');
  return { redirect: '/', authError: 'Auth is not available on the client side' };
}