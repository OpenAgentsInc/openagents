import { LoginForm } from "~/components/login-form"
import { Header } from "~/components/header"
import type { Route } from "./+types/login"

export function meta({ }: Route.MetaArgs) {
  return [
    { title: "Login - OpenAgents" },
    { name: "description", content: "Login to your OpenAgents account" },
  ];
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