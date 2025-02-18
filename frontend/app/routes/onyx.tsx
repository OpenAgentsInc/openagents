import type { Route } from "../+types/onyx";

export function meta({}: Route.MetaArgs) {
  return [
    { title: "OpenAgents - Mobile App" },
    { name: "description", content: "OpenAgents Mobile App - Onyx" },
  ];
}

export default function MobileApp() {
  return (
    <div id="content">
      <h1>Mobile App - Onyx</h1>
      {/* Add your mobile app content here */}
    </div>
  );
}