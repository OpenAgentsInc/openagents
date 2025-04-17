import { Header } from "~/components/header";
import { Badge } from "~/components/ui/badge";

export function Welcome() {
  return (
    <>
      <Header />
      <main className="fixed w-screen min-h-screen gap-4 flex flex-col items-center justify-center">
        <Badge variant="secondary" className="-mt-16 mb-2 select-none">Under construction</Badge>
        <h1 className="text-5xl font-bold select-none">OpenAgents</h1>
        <p className="text-lg select-none text-muted-foreground">Your agent dealer</p>
      </main>
    </>
  );
}
