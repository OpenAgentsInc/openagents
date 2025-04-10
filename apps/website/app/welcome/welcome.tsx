import { Link } from "react-router";
import { Header } from "~/components/header";

export function Welcome() {
  return (
    <>
      <Header />
      
      <main className="fixed w-screen min-h-screen gap-4 flex flex-col items-center justify-center">
        <h1 className="text-5xl font-bold select-none">OpenAgents</h1>
        <p className="text-lg select-none text-muted-foreground">Under construction</p>
      </main>
    </>
  );
}
