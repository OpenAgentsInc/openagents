import { useState } from "react"
import { useNavigate, Link } from "react-router"
import { cn } from "~/lib/utils"
import { Button } from "~/components/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "~/components/ui/card"
import { Input } from "~/components/ui/input"
import { Label } from "~/components/ui/label"
import { signIn } from "~/lib/auth-client"

export function LoginForm({
  className,
  ...props
}: React.ComponentProps<"div">) {
  const [email, setEmail] = useState("")
  const [password, setPassword] = useState("")
  const [error, setError] = useState<string | null>(null)
  const [isSubmitting, setIsSubmitting] = useState(false)

  const navigate = useNavigate()

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setError(null)

    // Basic validation
    if (!email || !password) {
      setError("Email and password are required")
      return
    }

    setIsSubmitting(true)

    try {
      // Use authClient.signIn directly as requested
      const { data, error: signInError } = await signIn.email(
        {
          email,
          password,
          callbackURL: "/"
        },
        {
          onRequest: () => {
            // Already handling with isSubmitting state
          },
          onSuccess: () => {
            // Redirect to dashboard or home page
            navigate("/")
          },
          onError: (ctx) => {
            console.error("Login error:", ctx.error)
            setError(ctx.error.message || "Invalid email or password")
            setIsSubmitting(false)
          }
        }
      )

      if (signInError) {
        setError(signInError.message || "Invalid email or password")
        setIsSubmitting(false)
      }
    } catch (error) {
      console.error("Login exception:", error)
      setError(error instanceof Error ? error.message : "An unknown error occurred")
      setIsSubmitting(false)
    }
  }

  return (
    <div className={cn("flex flex-col gap-6", className)} {...props}>
      <Card>
        <CardHeader>
          <CardTitle>Login to your account</CardTitle>
          <CardDescription>
            Enter your email below to login to your account
          </CardDescription>
        </CardHeader>
        <CardContent>
          <form
            className="space-y-8"
            onSubmit={handleSubmit}
          >
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
                  autoComplete="email"
                />
              </div>
              <div className="grid gap-3">
                <div className="flex items-center">
                  <Label htmlFor="password">Password</Label>
                  <a
                    href="#"
                    className="ml-auto inline-block text-sm underline-offset-4 hover:underline"
                  >
                    Forgot your password?
                  </a>
                </div>
                <Input
                  id="password"
                  type="password"
                  required
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  autoComplete="current-password"
                />
              </div>

              {/* Display validation/API errors */}
              {error && (
                <div className="p-3 rounded-md bg-destructive/10 text-destructive text-sm">
                  {error}
                </div>
              )}

              <div className="flex flex-col gap-3">
                <Button
                  type="submit"
                  className="w-full"
                  disabled={isSubmitting}
                >
                  {isSubmitting ? "Logging in..." : "Login"}
                </Button>
                <Button
                  variant="outline"
                  className="w-full"
                  type="button"
                  onClick={async () => {
                    try {
                      setIsSubmitting(true);
                      // Use better-auth client for social sign-in
                      await signIn.social({
                        provider: "github",
                        callbackURL: "/",
                      });
                    } catch (error) {
                      console.error("Social login error:", error);
                      setError(error instanceof Error ? error.message : "Failed to login");
                      setIsSubmitting(false);
                    }
                  }}
                  disabled={isSubmitting}
                >
                  Login with GitHub
                </Button>
                <Button
                  variant="outline"
                  className="w-full"
                  type="button"
                  onClick={async () => {
                    try {
                      setIsSubmitting(true);
                      // Use OAuth2 sign-in for ConsentKeys
                      await signIn.oauth2({
                        providerId: "consentkeys",
                        callbackURL: "/",
                      });
                    } catch (error) {
                      console.error("ConsentKeys login error:", error);
                      setError(error instanceof Error ? error.message : "Failed to login with ConsentKeys");
                      setIsSubmitting(false);
                    }
                  }}
                  disabled={isSubmitting}
                >
                  Sign in with ConsentKeys
                </Button>
              </div>
            </div>
            <div className="mt-4 text-center text-sm">
              Don&apos;t have an account?{" "}
              <Link to="/signup" className="underline underline-offset-4">
                Sign up
              </Link>
            </div>
          </form>
        </CardContent>
      </Card>
    </div>
  )
}
