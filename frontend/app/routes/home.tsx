import type { Route } from "../+types/home";

export function meta({}: Route.MetaArgs) {
  return [
    { title: "OpenAgents - Home" },
    { name: "description", content: "Welcome to OpenAgents" },
  ];
}

export default function Home() {
  return (
    <div className="flex justify-center mx-2 md:mx-6">
      <div className="w-[60rem] max-w-full my-6 px-4 py-6 border border-white">
        <div className="grid grid-cols-3 lg:grid-cols-6 gap-4 mb-6">
          {/* Navigation buttons will go here */}
        </div>
        <div id="content">
          <h1>Welcome to OpenAgents</h1>
          {/* Add your home page content here */}
        </div>
      </div>
    </div>
  );
}