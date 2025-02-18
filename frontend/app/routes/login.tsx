import type { Route } from "../+types/root";
import { Github } from "lucide-react";
import { Link } from "react-router";
import { LoginForm } from "../components/login-form";
import { Button } from "../components/ui/button";

export function meta({}: Route.MetaArgs) {
  return [
    { title: "OpenAgents - Login" },
    { name: "description", content: "Login to OpenAgents" },
  ];
}

export default function Login() {
  const handleGitHubLogin = () => {
    // Use window.location for auth routes to bypass React Router
    window.location.href = "/auth/github/login";
  };

  return (
    <div className="bg-background flex min-h-[75vh] flex-col items-center justify-center p-4 md:p-6">
      <div className="w-full max-w-sm">
        <LoginForm />
        <div className="mt-6">
          <div className="after:border-border relative text-center text-sm after:absolute after:inset-0 after:top-1/2 after:z-0 after:flex after:items-center after:border-t">
            <span className="bg-background text-muted-foreground relative z-10 px-2">
              or
            </span>
          </div>
          <div className="mt-6">
            <Button
              className="w-full"
              size="lg"
              variant="outline"
              onClick={handleGitHubLogin}
            >
              <Github className="mr-2 h-5 w-5" />
              Continue with GitHub
            </Button>
          </div>
          <div className="text-muted-foreground *:[a]:hover:text-primary text-center text-xs text-balance mt-6 *:[a]:underline *:[a]:underline-offset-4">
            By clicking continue, you agree to our{" "}
            <Link to="/terms">Terms of Service</Link> and{" "}
            <Link to="/privacy">Privacy Policy</Link>.
          </div>
        </div>
      </div>
    </div>
  );
}
