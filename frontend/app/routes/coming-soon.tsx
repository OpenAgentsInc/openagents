import type { Route } from "../+types/coming-soon";

export function meta({}: Route.MetaArgs) {
  return [
    { title: "OpenAgents - Coming Soon" },
    { name: "description", content: "Coming Soon - OpenAgents" },
  ];
}

export default function ComingSoon() {
  return (
    <div className="container mx-auto px-4 max-w-4xl">
      <h1 className="text-lg font-bold mb-6">Coming Soon</h1>

      <div className="space-y-8">
        <p className="text-sm mb-6">
          We're building the next generation of truly open & useful AI. Here's
          what's coming:
        </p>

        <div className="space-y-6">
          <div className="border-l-4 border-white pl-4">
            <h3 className="text-base font-semibold mb-3">Agent Dashboard</h3>
            <p className="text-sm">
              A powerful interface for creating, training, and deploying your
              own AI agents using our open protocols.
            </p>
          </div>

          <div className="border-l-4 border-white pl-4">
            <h3 className="text-base font-semibold mb-3">
              Open Knowledge Graph
            </h3>
            <p className="text-sm">
              A decentralized knowledge base built by both humans and AI agents,
              with contributors earning bitcoin rewards.
            </p>
          </div>

          <div className="border-l-4 border-white pl-4">
            <h3 className="text-base font-semibold mb-3">Bitcoin Rewards</h3>
            <p className="text-sm">
              Earn bitcoin for contributing to the ecosystem, whether you're
              building agents or improving the knowledge graph.
            </p>
          </div>

          <div className="border-l-4 border-white pl-4">
            <h3 className="text-base font-semibold mb-3">Nostr Login</h3>
            <p className="text-sm">
              Seamless authentication using your Nostr identity, enabling true
              ownership of your data and interactions.
            </p>
          </div>
        </div>
      </div>
    </div>
  );
}
