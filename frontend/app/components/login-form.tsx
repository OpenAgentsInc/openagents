import { Mail } from "lucide-react";
import { useState } from "react";
import { Button } from "~/components/ui/button";
import { Input } from "~/components/ui/input";
import { Label } from "~/components/ui/label";
import { cn } from "~/lib/utils";

export function LoginForm({
  className,
  ...props
}: React.ComponentProps<"div">) {
  const [email, setEmail] = useState("");
  const [isExistingUser, setIsExistingUser] = useState<boolean | null>(null);
  const [showPassword, setShowPassword] = useState(false);
  const [password, setPassword] = useState("");
  const [isLoading, setIsLoading] = useState(false);

  const checkEmail = async (email: string) => {
    if (!email) return;
    
    setIsLoading(true);
    try {
      const response = await fetch(`/api/users/check-email?email=${encodeURIComponent(email)}`);
      const data = await response.json();
      setIsExistingUser(data.exists);
      setShowPassword(data.exists);
    } catch (error) {
      console.error("Error checking email:", error);
    } finally {
      setIsLoading(false);
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!email) return;

    if (isExistingUser === null) {
      await checkEmail(email);
      return;
    }

    // Use window.location for auth routes to bypass React Router
    if (!isExistingUser) {
      // New user - go to signup
      window.location.href = `/auth/scramble/signup?email=${encodeURIComponent(email)}`;
    } else {
      // Existing user - go to login
      window.location.href = `/auth/scramble/login?email=${encodeURIComponent(email)}`;
    }
  };

  return (
    <div className={cn("flex flex-col gap-6", className)} {...props}>
      <form onSubmit={handleSubmit}>
        <div className="flex flex-col gap-6">
          <div className="flex flex-col items-center gap-2">
            <span className="text-5xl select-none">⏻</span>
            <h1 className="text-xl font-bold">Welcome to OpenAgents</h1>
            <div className="text-center text-sm">
              Sign up or log in to continue
            </div>
          </div>
          <div className="flex flex-col gap-6">
            <div className="grid gap-3">
              <Label htmlFor="email">Email</Label>
              <Input
                id="email"
                type="email"
                placeholder="m@example.com"
                required
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                onBlur={() => email && checkEmail(email)}
                disabled={isLoading}
              />
            </div>
            {showPassword && (
              <div className="grid gap-3">
                <Label htmlFor="password">Password</Label>
                <Input
                  id="password"
                  type="password"
                  required
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  disabled={isLoading}
                />
              </div>
            )}
            <Button type="submit" className="w-full" disabled={isLoading}>
              <Mail className="mr-2 h-5 w-5" />
              {isLoading ? "Checking..." : isExistingUser === null ? "Continue with email" : isExistingUser ? "Log in" : "Sign up"}
            </Button>
          </div>
        </div>
      </form>
    </div>
  );
}