import { useState } from "react"
import { useNavigate } from "react-router"
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
import { Link } from "react-router"
import { signUp } from "~/lib/auth-client"

export function SignupForm({
  className,
  ...props
}: React.ComponentProps<"div">) {
  const [email, setEmail] = useState("")
  const [password, setPassword] = useState("")
  const [confirmPassword, setConfirmPassword] = useState("")
  const [error, setError] = useState<string | null>(null)
  const [success, setSuccess] = useState(false)
  const [isSubmitting, setIsSubmitting] = useState(false)
  
  const navigate = useNavigate()
  
  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setError(null)
    setSuccess(false)
    
    // Basic validation
    if (!email || !password || !confirmPassword) {
      setError("All fields are required")
      return
    }
    
    if (password !== confirmPassword) {
      setError("Passwords do not match")
      return
    }
    
    if (password.length < 8) {
      setError("Password must be at least 8 characters long")
      return
    }
    
    setIsSubmitting(true)
    
    try {
      // Use authClient directly as requested
      const { data, error: signupError } = await signUp.email(
        {
          email,
          password,
          // Add a name since it's required by the schema
          name: email.split('@')[0], // Use part of email as name
          callbackURL: "/"
        },
        {
          onRequest: () => {
            // Already handling with isSubmitting state
          },
          onSuccess: () => {
            setSuccess(true)
            // Wait a moment to show the success message before redirecting to home
            setTimeout(() => {
              navigate("/")
            }, 1500)
          },
          onError: (ctx) => {
            console.error("Signup error:", ctx.error)
            setError(ctx.error.message || "Failed to create account")
            setIsSubmitting(false)
          }
        }
      )
      
      if (signupError) {
        setError(signupError.message || "Failed to create account")
        setIsSubmitting(false)
      }
    } catch (error) {
      console.error("Signup exception:", error)
      setError(error instanceof Error ? error.message : "An unknown error occurred")
      setIsSubmitting(false)
    }
  }

  return (
    <div className={cn("flex flex-col gap-6", className)} {...props}>
      <Card>
        <CardHeader>
          <CardTitle>Create an account</CardTitle>
          <CardDescription>
            Enter your email and password to create your account
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
                <Label htmlFor="password">Password</Label>
                <Input 
                  id="password" 
                  type="password" 
                  required 
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  autoComplete="new-password"
                />
              </div>
              <div className="grid gap-3">
                <Label htmlFor="confirmPassword">Confirm Password</Label>
                <Input 
                  id="confirmPassword" 
                  type="password" 
                  required 
                  value={confirmPassword}
                  onChange={(e) => setConfirmPassword(e.target.value)}
                  autoComplete="new-password"
                />
              </div>
              
              {/* Display validation/API errors */}
              {error && (
                <div className="p-3 rounded-md bg-destructive/10 text-destructive text-sm">
                  {error}
                </div>
              )}
              
              {/* Display success message */}
              {success && (
                <div className="p-3 rounded-md bg-green-500/10 text-green-600 dark:text-green-400 text-sm">
                  Account created successfully! Redirecting to home page...
                </div>
              )}
              
              <div className="flex flex-col gap-3">
                <Button 
                  type="submit" 
                  className="w-full"
                  disabled={isSubmitting}
                >
                  {isSubmitting ? "Creating Account..." : "Create Account"}
                </Button>
                <Button 
                  variant="outline" 
                  className="w-full"
                  type="button"
                  onClick={async () => {
                    try {
                      setIsSubmitting(true);
                      // Use better-auth client directly for social sign-up
                      await signUp.social({
                        provider: "github", 
                        callbackURL: "/",
                      });
                    } catch (error) {
                      console.error("Social signup error:", error);
                      setError(error instanceof Error ? error.message : "Failed to sign up");
                      setIsSubmitting(false);
                    }
                  }}
                  disabled={isSubmitting}
                >
                  Sign up with GitHub
                </Button>
                <Button 
                  variant="outline" 
                  className="w-full"
                  type="button"
                  onClick={async () => {
                    try {
                      setIsSubmitting(true);
                      // Use social sign-up for ConsentKeys
                      await signUp.social({
                        provider: "consentkeys", 
                        callbackURL: "/",
                      });
                    } catch (error) {
                      console.error("ConsentKeys signup error:", error);
                      setError(error instanceof Error ? error.message : "Failed to sign up with ConsentKeys");
                      setIsSubmitting(false);
                    }
                  }}
                  disabled={isSubmitting}
                >
                  Sign up with ConsentKeys
                </Button>
              </div>
            </div>
            <div className="mt-4 text-center text-sm">
              Already have an account?{" "}
              <Link to="/login" className="underline underline-offset-4">
                Log in
              </Link>
            </div>
          </form>
        </CardContent>
      </Card>
    </div>
  )
}