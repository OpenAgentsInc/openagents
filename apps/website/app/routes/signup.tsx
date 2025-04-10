import { SignupForm } from "~/components/signup-form"
import { Header } from "~/components/header"
import type { Route } from "./+types/signup"
import { redirect } from "react-router"
import type { ActionFunctionArgs } from "react-router"
import { auth } from "~/lib/auth"

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
    // Debug auth object structure
    console.log("Auth object structure:", Object.keys(auth));
    
    // Use better-auth to create the account with proper checking
    if (typeof auth.signUp !== 'function') {
      console.error("Auth methods:", Object.keys(auth));
      throw new Error("Auth signUp method is not available");
    }
    
    // Call the signUp method directly - it should be on the auth object itself
    const result = await auth.signUp({
      email,
      password,
      // Additional fields could be added here if needed
      // name: formData.get("name") as string,
    });
    
    console.log("Signup result:", result);
    
    if (result.error) {
      console.error("Signup error:", result.error);
      return { 
        success: false, 
        error: result.error.message || "Failed to create account" 
      };
    }
    
    // Check if auto sign-in is enabled in the better-auth config
    if (result.data) {
      // Get the session to see if user was auto-signed in
      const session = await auth.getSession({
        headers: request.headers,
      });
      
      console.log("Session after signup:", session);
      
      if (session?.user) {
        // User was auto-signed in, redirect to home or dashboard
        return redirect("/");
      } else {
        // User was not auto-signed in, return success to redirect to login
        return { success: true };
      }
    } else {
      return { success: false, error: "Failed to create account" };
    }
  } catch (error) {
    console.error("Signup exception:", error);
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