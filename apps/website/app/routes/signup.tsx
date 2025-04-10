import { SignupForm } from "~/components/signup-form"
import { Header } from "~/components/header"
import type { Route } from "./+types/signup"
import { redirect } from "react-router"
import type { ActionFunctionArgs } from "react-router"

export function meta({ }: Route.MetaArgs) {
  return [
    { title: "Sign Up - OpenAgents" },
    { name: "description", content: "Create your OpenAgents account" },
  ];
}

export async function action({ request }: ActionFunctionArgs) {
  const formData = await request.formData();
  const email = formData.get("email") as string;
  const password = formData.get("password") as string;
  const confirmPassword = formData.get("confirmPassword") as string;

  // Validate the form data
  if (!email || !password || !confirmPassword) {
    return { success: false, error: "All fields are required" };
  }

  if (password !== confirmPassword) {
    return { success: false, error: "Passwords do not match" };
  }

  if (password.length < 8) {
    return { success: false, error: "Password must be at least 8 characters long" };
  }

  try {
    // In a real implementation with better-auth, we would use:
    // import { auth } from "~/lib/auth";
    // const { data, error } = await auth.api.signUp.email({ 
    //   email, 
    //   password,
    //   // optional name or other profile fields
    // });
    
    // Note: In the front-end, the signUp function from better-auth/react would be used:
    // import { signUp } from "~/lib/auth-client";
    // const { data, error } = await signUp.email({ email, password });
    
    // Simulate successful signup
    const success = true;
    
    if (success) {
      // In a real implementation, we might:
      // 1. Auto-sign in the user (if enabled in better-auth config)
      // 2. Redirect to login or dashboard
      // 3. Send a verification email
      
      // For this demo, just return success and redirect from the client side
      return { success: true };
    } else {
      return { success: false, error: "Failed to create account" };
    }
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : "An unknown error occurred"
    };
  }
}

export default function Signup() {
  return (
    <>
      <Header showNewAgentButton={false} />
      
      <div className="flex min-h-svh w-full items-center justify-center p-6 pt-24 md:p-10 md:pt-24">
        <div className="w-full max-w-sm">
          <SignupForm />
        </div>
      </div>
    </>
  )
}