import type { Route } from "./+types/spawn";

export function meta({}: Route.MetaArgs) {
  return [
    { title: "Spawn Agent - OpenAgents" },
    { name: "description", content: "Spawn a new coding agent" },
  ];
}

export default function Spawn() {
  return (
    <main className="w-full p-8">
      <h1 className="text-4xl font-bold mb-6">Spawn a coding agent</h1>
      {/* Agent creation content will go here */}
    </main>
  );
}