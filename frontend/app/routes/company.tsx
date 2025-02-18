import type { Route } from "../+types/company";

export function meta({}: Route.MetaArgs) {
  return [
    { title: "OpenAgents - Company" },
    { name: "description", content: "About OpenAgents" },
  ];
}

export default function Company() {
  return (
    <div className="flex justify-center mx-2 md:mx-6">
      <div className="w-[60rem] max-w-full my-6 px-4 py-6 border border-white">
        <div className="grid grid-cols-3 lg:grid-cols-6 gap-4 mb-6">
          {/* Navigation buttons will go here */}
        </div>
        <div id="content">
          <h1>Company</h1>
          {/* Add your company content here */}
        </div>
      </div>
    </div>
  );
}