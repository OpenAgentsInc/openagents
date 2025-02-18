import type { Route } from "../+types/home";

export function meta({}: Route.MetaArgs) {
  return [
    { title: "OpenAgents - Home" },
    { name: "description", content: "Welcome to OpenAgents" },
  ];
}

export default function Home() {
  return (
    <div id="content">
      <h1>Welcome to OpenAgents</h1>
      {/* Add your home page content here */}
    </div>
  );
}