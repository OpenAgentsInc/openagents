import { LoginForm } from "~/components/login-form"
import { Header } from "~/components/header"
import type { Route } from "./+types/login"
import { redirect } from "react-router"
import type { ActionFunctionArgs } from "react-router"
import { auth } from "~/lib/auth"

export function meta({ }: Route.MetaArgs) {
  return [
    { title: "Login - OpenAgents" },
    { name: "description", content: "Login to your OpenAgents account" },
  ];
}

export async function action({ request }: ActionFunctionArgs) {
  const formData = await request.formData();
  const email = formData.get("email") as string;
  const password = formData.get("password") as string;

  // Validate the form data
  if (!email || !password) {
    return { success: false, error: "Email and password are required" };
  }

  try {
    // Debug auth object structure
    console.log("Auth object structure for login:", Object.keys(auth));
    
    // Use better-auth to sign in with proper checking
    if (typeof auth.signIn !== 'function') {
      console.error("Auth methods:", Object.keys(auth));
      throw new Error("Auth signIn method is not available");
    }
    
    // Call the signIn method directly
    const result = await auth.signIn({
      email,
      password,
      // Redirect to home page after successful login
      callbackURL: "/"
    });
    
    console.log("Login result:", result);
    
    if (result.error) {
      console.error("Login error:", result.error);
      return { 
        success: false, 
        error: result.error.message || "Invalid email or password" 
      };
    }
    
    if (result.data) {
      // Login was successful, redirect to home page
      return redirect("/");
    } else {
      return { success: false, error: "Login failed" };
    }
  } catch (error) {
    console.error("Login exception:", error);
    return {
      success: false,
      error: error instanceof Error ? error.message : "An unknown error occurred"
    };
  }
}

export default function Login() {
  return (
    <>
      <Header showNewAgentButton={false} />
      
      <div className="flex min-h-svh w-full items-center justify-center p-6 pt-24 md:p-10 md:pt-24">
        <div className="w-full max-w-sm">
          <LoginForm />
        </div>
      </div>
    </>
  )
}
