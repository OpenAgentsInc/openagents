import { useAuthActions } from "@convex-dev/auth/react";
import { useState, useEffect } from "react";

export function SignIn() {
  const { signIn, signOut } = useAuthActions();
  const [flow, setFlow] = useState<"signIn" | "signUp">("signIn");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  // Clear any stale auth state on mount
  useEffect(() => {
    console.log("[SignIn] Component mounted - user is unauthenticated");
    // Force sign out to clear any stale tokens
    void signOut();
  }, [signOut]);

  return (
    <div className="flex flex-col gap-8 w-full max-w-lg mx-auto h-screen justify-center items-center px-4 bg-zinc-900">
      <div className="text-center flex flex-col items-center gap-4">
        <h1 className="text-3xl font-bold text-zinc-100">
          OpenAgents
        </h1>
        <p className="text-zinc-400">
          Sign in or sign up to access your agents and chats across all devices.
        </p>
      </div>
      <form
        className="flex flex-col gap-4 w-full bg-zinc-800 p-8 shadow-xl border border-zinc-700"
        onSubmit={(e) => {
          e.preventDefault();
          setLoading(true);
          setError(null);
          const formData = new FormData(e.target as HTMLFormElement);
          formData.set("flow", flow);
          void signIn("password", formData)
            .catch((error) => {
              setError(error.message);
              setLoading(false);
            })
            .then(() => {
              // Auth successful, component will unmount as user is now authenticated
              setLoading(false);
            });
        }}
      >
        <input
          className="bg-zinc-900 text-white p-3 border border-zinc-600 focus:border-zinc-500 focus:ring-2 focus:ring-zinc-700 outline-none transition-all placeholder:text-zinc-500"
          type="email"
          name="email"
          placeholder="Email"
          required
        />
        <div className="flex flex-col gap-1">
          <input
            className="bg-zinc-900 text-white p-3 border border-zinc-600 focus:border-zinc-500 focus:ring-2 focus:ring-zinc-700 outline-none transition-all placeholder:text-zinc-500"
            type="password"
            name="password"
            placeholder="Password"
            minLength={8}
            required
          />
          {flow === "signUp" && (
            <p className="text-xs text-zinc-400 px-1">
              Password must be at least 8 characters
            </p>
          )}
        </div>
        <button
          className="bg-zinc-700 hover:bg-zinc-600 text-white font-semibold py-3 shadow-md hover:shadow-lg transition-all duration-200 hover:scale-[1.02] active:scale-[0.98] cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:scale-100"
          type="submit"
          disabled={loading}
        >
          {loading ? "Loading..." : flow === "signIn" ? "Sign in" : "Sign up"}
        </button>
        <div className="flex flex-row gap-2 text-sm justify-center">
          <span className="text-zinc-400">
            {flow === "signIn"
              ? "Don't have an account?"
              : "Already have an account?"}
          </span>
          <span
            className="text-zinc-300 hover:text-zinc-100 font-medium underline decoration-2 underline-offset-2 hover:no-underline cursor-pointer transition-colors"
            onClick={() => setFlow(flow === "signIn" ? "signUp" : "signIn")}
          >
            {flow === "signIn" ? "Sign up" : "Sign in"}
          </span>
        </div>
        {error && (
          <div className="bg-red-500/10 border border-red-500/30 p-4">
            <p className="text-red-400 font-medium text-sm break-words">
              Error: {error}
            </p>
          </div>
        )}
      </form>
    </div>
  );
}
