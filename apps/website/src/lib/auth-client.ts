/**
 * Better Auth client for OpenAgents website (browser).
 * Use for signIn, signOut, getSession in client-side scripts.
 * See: https://better-auth.com/docs/concepts/client
 */

import { createAuthClient } from "better-auth/client";

function getBaseURL(): string {
	if (typeof import.meta !== "undefined" && import.meta.env?.PUBLIC_SITE_URL) {
		return String(import.meta.env.PUBLIC_SITE_URL);
	}
	if (typeof window !== "undefined") {
		return window.location.origin;
	}
	return "https://openagents.com";
}

export const authClient = createAuthClient({
	baseURL: getBaseURL(),
});

export const { signIn, signOut, signUp, getSession } = authClient;
