import type { Route } from "../+types/company";

export function meta({ }: Route.MetaArgs) {
  return [
    { title: "OpenAgents - Company" },
    { name: "description", content: "About OpenAgents" },
  ];
}

export default function Company() {
  return (
    <div id="content">
      <h1>Company</h1>
      {/* Add your company content here */}
    </div>
  );
}
