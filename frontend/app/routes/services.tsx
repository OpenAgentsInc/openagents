import type { Route } from "../+types/services";

export function meta({}: Route.MetaArgs) {
  return [
    { title: "OpenAgents - Services" },
    { name: "description", content: "OpenAgents Business Services" },
  ];
}

export default function Services() {
  return (
    <div id="content">
      <h1>Services</h1>
      {/* Add your services content here */}
    </div>
  );
}