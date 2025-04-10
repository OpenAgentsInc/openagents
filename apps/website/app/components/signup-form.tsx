import { useState, useEffect } from "react"
import { Form, useActionData, useNavigate } from "react-router"
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

export function SignupForm({
  className,
  ...props
}: React.ComponentProps<"div">) {
  const [email, setEmail] = useState("")
  const [password, setPassword] = useState("")
  const [confirmPassword, setConfirmPassword] = useState("")
  const [error, setError] = useState<string | null>(null)
  const [isSubmitting, setIsSubmitting] = useState(false)
  
  const navigate = useNavigate()
  const actionData = useActionData<{
    success?: boolean;
    error?: string;
  }>()
  
  // Handle redirect after successful signup
  useEffect(() => {
    if (actionData?.success) {
      // Wait a moment to show the success message before redirecting
      const timer = setTimeout(() => {
        navigate("/login");
      }, 1500);
      
      return () => clearTimeout(timer);
    }
  }, [actionData, navigate]);

  const handleBeforeSubmit = () => {
    setError(null)
    
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
                <Label htmlFor="password">Password</Label>
                <Input 
                  id="password" 
                  name="password" 
                  type="password" 
                  required 
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                />
              </div>
              <div className="grid gap-3">
                <Label htmlFor="confirmPassword">Confirm Password</Label>
                <Input 
                  id="confirmPassword" 
                  name="confirmPassword" 
                  type="password" 
                  required 
                  value={confirmPassword}
                  onChange={(e) => setConfirmPassword(e.target.value)}
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
              
              {/* Display success message */}
              {actionData?.success && (
                <div className="p-3 rounded-md bg-green-500/10 text-green-600 dark:text-green-400 text-sm">
                  Account created successfully! Redirecting to login...
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
                <Button variant="outline" className="w-full">
                  Sign up with Google
                </Button>
              </div>
            </div>
            <div className="mt-4 text-center text-sm">
              Already have an account?{" "}
              <Link to="/login" className="underline underline-offset-4">
                Log in
              </Link>
            </div>
          </Form>
        </CardContent>
      </Card>
    </div>
  )
}