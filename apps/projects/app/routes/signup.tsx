import { SignupForm } from "~/components/signup-form"
import { Header } from "~/components/header"
import type { Route } from "./+types/signup"

export function meta({ }: Route.MetaArgs) {
  return [
    { title: "Sign Up - OpenAgents" },
    { name: "description", content: "Create your OpenAgents account" },
  ];
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