import { Mail } from "lucide-react";
import { useEffect, useState } from "react";
import { Alert, AlertDescription } from "~/components/ui/alert";
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
  const [error, setError] = useState<string | null>(null);

  // Log state changes
  useEffect(() => {
    console.log("[LoginForm] State updated:", {
      email,
      isExistingUser,
      showPassword,
      isLoading,
      error,
    });
  }, [email, isExistingUser, showPassword, isLoading, error]);

  const checkEmail = async (email: string) => {
    if (!email) return;

    console.log("[LoginForm] Checking email:", email);
    setIsLoading(true);
    try {
      const url = `/api/users/check-email?email=${encodeURIComponent(email)}`;
      console.log("[LoginForm] Making API request to:", url);

      const response = await fetch(url);
      const data = await response.json();
      console.log("[LoginForm] Email check response:", data);

      setIsExistingUser(data.exists);
      setShowPassword(true); // Show password field regardless of user existence
    } catch (error) {
      console.error("[LoginForm] Error checking email:", error);
    } finally {
      setIsLoading(false);
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    console.log("[LoginForm] Form submitted");
    setError(null); // Clear any previous errors

    if (!email) {
      console.log("[LoginForm] No email provided, stopping submission");
      setError("Email is required");
      return;
    }

    // First check if user exists if we don't know yet
    if (isExistingUser === null) {
      console.log("[LoginForm] User existence unknown, checking email first");
      await checkEmail(email);
      return;
    }

    // Require password before proceeding
    if (!password) {
      console.log("[LoginForm] Password required but not provided");
      setError("Password is required");
      return;
    }

    setIsLoading(true);
    console.log("[LoginForm] Starting authentication process");

    const endpoint = isExistingUser
      ? "/auth/scramble/login"
      : "/auth/scramble/signup";
    console.log(
      `[LoginForm] Using endpoint: ${endpoint} for ${isExistingUser ? "login" : "signup"}`,
    );

    try {
      console.log("[LoginForm] Sending auth request");
      const response = await fetch(endpoint, {
        method: "POST",
        headers: {
          "Content-Type": "application/x-www-form-urlencoded",
        },
        body: new URLSearchParams({
          email,
          password: password || "",
        }),
      });

      if (response.ok) {
        const data = await response.json();
        console.log("[LoginForm] Auth response successful:", data);
        if (data.error) {
          setError(data.error);
          return;
        }
        // Handle successful response - should get OAuth URL
        if (data.url) {
          console.log("[LoginForm] Redirecting to:", data.url);
          window.location.href = data.url;
        }
      } else {
        const errorText = await response.text();
        console.error("[LoginForm] Auth error response:", {
          status: response.status,
          statusText: response.statusText,
          body: errorText,
        });
        setError("Authentication failed. Please try again.");
      }
    } catch (error) {
      console.error("[LoginForm] Submit error:", error);
      setError("An unexpected error occurred. Please try again.");
    } finally {
      setIsLoading(false);
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
          {error && (
            <Alert variant="destructive">
              <AlertDescription>{error}</AlertDescription>
            </Alert>
          )}
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
              {isLoading
                ? "Checking..."
                : isExistingUser === null
                  ? "Continue with email"
                  : isExistingUser
                    ? "Log in"
                    : "Sign up"}
            </Button>
          </div>
        </div>
      </form>
    </div>
  );
}
