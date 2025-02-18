import type { Route } from "../+types/coming-soon";

export function meta({ }: Route.MetaArgs) {
  return [
    { title: "OpenAgents - Coming Soon" },
    { name: "description", content: "Coming Soon - OpenAgents" },
  ];
}

export default function ComingSoon() {
  return (
    <div id="content">
      <h1>Coming Soon</h1>
      {/* Add your coming soon content here */}
    </div>
  );
}
