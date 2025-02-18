import type { Route } from "../+types/coming-soon";

export function meta({}: Route.MetaArgs) {
  return [
    { title: "OpenAgents - Coming Soon" },
    { name: "description", content: "Coming Soon - OpenAgents" },
  ];
}

export default function ComingSoon() {
  return (
    <div className="flex justify-center mx-2 md:mx-6">
      <div className="w-[60rem] max-w-full my-6 px-4 py-6 border border-white">
        <div className="grid grid-cols-3 lg:grid-cols-6 gap-4 mb-6">
          {/* Navigation buttons will go here */}
        </div>
        <div id="content">
          <h1>Coming Soon</h1>
          {/* Add your coming soon content here */}
        </div>
      </div>
    </div>
  );
}