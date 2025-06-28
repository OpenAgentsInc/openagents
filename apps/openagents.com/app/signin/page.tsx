"use client";

import { useAuthActions } from "@convex-dev/auth/react";
import { useRouter } from "next/navigation";
import { useState } from "react";
import { Github } from "lucide-react";

export default function SignIn() {
  const { signIn } = useAuthActions();
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const router = useRouter();

  const handleGitHubSignIn = async () => {
    setIsLoading(true);
    setError(null);
    
    try {
      await signIn("github");
      // Redirect will happen automatically after successful OAuth
    } catch (error) {
      console.error("GitHub sign-in error:", error);
      setError(error instanceof Error ? error.message : "Failed to sign in with GitHub");
      setIsLoading(false);
    }
  };

  return (
    <div className="flex flex-col gap-8 w-96 mx-auto h-screen justify-center items-center">
      <div className="text-center">
        <h1 className="text-2xl font-bold mb-2">Sign in to OpenAgents</h1>
        <p className="text-gray-600">Chat your apps into existence. Deploy in 60 seconds.</p>
      </div>
      
      <div className="flex flex-col gap-4 w-full">
        <button
          onClick={handleGitHubSignIn}
          disabled={isLoading}
          className="flex items-center justify-center gap-3 w-full bg-foreground text-background rounded-md px-4 py-3 font-medium hover:opacity-90 transition-opacity disabled:opacity-50 disabled:cursor-not-allowed"
        >
          <Github size={20} />
          <span>{isLoading ? "Connecting..." : "Continue with GitHub"}</span>
        </button>
        
        {error && (
          <div className="bg-red-500/20 border-2 border-red-500/50 rounded-md p-3">
            <p className="text-foreground font-mono text-sm">
              Error: {error}
            </p>
          </div>
        )}
      </div>
      
      <p className="text-sm text-gray-500 text-center max-w-sm">
        By signing in, you agree to our Terms of Service and Privacy Policy.
      </p>
    </div>
  );
}