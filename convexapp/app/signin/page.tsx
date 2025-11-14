"use client";

import { useAuthActions } from "@convex-dev/auth/react";
import { useRouter } from "next/navigation";
import { useState } from "react";
import Image from "next/image";

export default function SignIn() {
  const { signIn } = useAuthActions();
  const [flow, setFlow] = useState<"signIn" | "signUp">("signIn");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const router = useRouter();
  return (
    <div className="flex flex-col gap-8 w-full max-w-lg mx-auto h-screen justify-center items-center px-4">
      <div className="text-center flex flex-col items-center gap-4">
        <div className="flex items-center gap-6">
          <Image
            src="/convex.svg"
            alt="Convex Logo"
            width={90}
            height={90}
          />
          <div className="w-px h-20 bg-slate-300 dark:bg-slate-600"></div>
          <Image
            src="/nextjs-icon-light-background.svg"
            alt="Next.js Logo"
            width={90}
            height={90}
            className="dark:hidden"
          />
          <Image
            src="/nextjs-icon-dark-background.svg"
            alt="Next.js Logo"
            width={90}
            height={90}
            className="hidden dark:block"
          />
        </div>
        <h1 className="text-3xl font-bold text-slate-800 dark:text-slate-200">
          Convex + Next.js + Convex Auth
        </h1>
        <p className="text-slate-600 dark:text-slate-400">
          This demo uses Convex Auth for authentication, so you will need to
          sign in or sign up to access the demo.
        </p>
      </div>
      <form
        className="flex flex-col gap-4 w-full bg-slate-100 dark:bg-slate-800 p-8 rounded-2xl shadow-xl border border-slate-300 dark:border-slate-600"
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
              router.push("/");
            });
        }}
      >
        <input
          className="bg-white dark:bg-slate-900 text-foreground rounded-lg p-3 border border-slate-300 dark:border-slate-600 focus:border-slate-500 dark:focus:border-slate-400 focus:ring-2 focus:ring-slate-200 dark:focus:ring-slate-700 outline-none transition-all placeholder:text-slate-400"
          type="email"
          name="email"
          placeholder="Email"
          required
        />
        <div className="flex flex-col gap-1">
          <input
            className="bg-white dark:bg-slate-900 text-foreground rounded-lg p-3 border border-slate-300 dark:border-slate-600 focus:border-slate-500 dark:focus:border-slate-400 focus:ring-2 focus:ring-slate-200 dark:focus:ring-slate-700 outline-none transition-all placeholder:text-slate-400"
            type="password"
            name="password"
            placeholder="Password"
            minLength={8}
            required
          />
          {flow === "signUp" && (
            <p className="text-xs text-slate-500 dark:text-slate-400 px-1">
              Password must be at least 8 characters
            </p>
          )}
        </div>
        <button
          className="bg-slate-700 hover:bg-slate-800 dark:bg-slate-600 dark:hover:bg-slate-500 text-white font-semibold rounded-lg py-3 shadow-md hover:shadow-lg transition-all duration-200 hover:scale-[1.02] active:scale-[0.98] cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:scale-100"
          type="submit"
          disabled={loading}
        >
          {loading ? "Loading..." : flow === "signIn" ? "Sign in" : "Sign up"}
        </button>
        <div className="flex flex-row gap-2 text-sm justify-center">
          <span className="text-slate-600 dark:text-slate-400">
            {flow === "signIn"
              ? "Don't have an account?"
              : "Already have an account?"}
          </span>
          <span
            className="text-slate-700 dark:text-slate-300 hover:text-slate-900 dark:hover:text-slate-100 font-medium underline decoration-2 underline-offset-2 hover:no-underline cursor-pointer transition-colors"
            onClick={() => setFlow(flow === "signIn" ? "signUp" : "signIn")}
          >
            {flow === "signIn" ? "Sign up" : "Sign in"}
          </span>
        </div>
        {error && (
          <div className="bg-rose-500/10 border border-rose-500/30 dark:border-rose-500/50 rounded-lg p-4">
            <p className="text-rose-700 dark:text-rose-300 font-medium text-sm break-words">
              Error: {error}
            </p>
          </div>
        )}
      </form>
    </div>
  );
}
