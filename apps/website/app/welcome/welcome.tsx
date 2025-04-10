import { Button } from "~/components/ui/button";

export function Welcome() {
  return (
    <main className="fixed w-screen min-h-screen gap-4 flex flex-col items-center justify-center">
      <h1 className="text-4xl font-bold select-none">OpenAgents</h1>
      <p className="text-lg select-none text-muted-foreground mb-2">Under construction</p>
      <Button variant="outline" size="lg">Spawn a coding agent</Button>
    </main>
  );
}
