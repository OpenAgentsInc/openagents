import { useState } from "react"
import { Form, useActionData, useNavigate, Link } from "react-router"
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
  const actionData = useActionData<{
    success?: boolean;
    error?: string;
  }>()

  const handleBeforeSubmit = () => {
    setError(null)
    
    // Basic validation
    if (!email || !password) {
      setError("Email and password are required")
      return
    }
    
    setIsSubmitting(true)
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
          <Form 
            method="post" 
            className="space-y-8"
            onSubmit={handleBeforeSubmit}
            preventScrollReset
          >
            <div className="flex flex-col gap-6">
              <div className="grid gap-3">
                <Label htmlFor="email">Email</Label>
                <Input
                  id="email"
                  name="email"
                  type="email"
                  placeholder="m@example.com"
                  required
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
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
                  name="password"
                  type="password" 
                  required 
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                />
              </div>
              
              {/* Display validation errors */}
              {error && (
                <div className="p-3 rounded-md bg-destructive/10 text-destructive text-sm">
                  {error}
                </div>
              )}
              
              {/* Display API errors */}
              {actionData?.error && (
                <div className="p-3 rounded-md bg-destructive/10 text-destructive text-sm">
                  {actionData.error}
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
                {/* Social login buttons will be implemented later 
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
                */}
              </div>
            </div>
            <div className="mt-4 text-center text-sm">
              Don&apos;t have an account?{" "}
              <Link to="/signup" className="underline underline-offset-4">
                Sign up
              </Link>
            </div>
          </Form>
        </CardContent>
      </Card>
    </div>
  )
}
