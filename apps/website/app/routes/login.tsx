import { LoginForm } from "~/components/login-form"
import { Header } from "~/components/header"
import type { Route } from "./+types/login"
import { redirect } from "react-router"
import type { ActionFunctionArgs } from "react-router"

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
    // In a real implementation with better-auth, we would use:
    // import { auth } from "~/lib/auth";
    // const { data, error } = await auth.api.signIn.email({ 
    //   email, 
    //   password,
    //   callbackURL: "/"
    // });
    
    // Note: In the front-end, the signIn function from better-auth/react would be used:
    // import { signIn } from "~/lib/auth-client";
    // const { data, error } = await signIn.email({ email, password, callbackURL: "/" });
    
    // For the demo, simulate successful login
    const success = true;
    
    if (success) {
      // Redirect to home page after login
      return redirect("/");
    } else {
      return { success: false, error: "Invalid email or password" };
    }
  } catch (error) {
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
