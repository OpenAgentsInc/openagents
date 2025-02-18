import type { Route } from "../+types/company";

export function meta({}: Route.MetaArgs) {
  return [
    { title: "OpenAgents - Company" },
    { name: "description", content: "About OpenAgents" },
  ];
}

export default function Company() {
  return (
    <div className="container mx-auto px-4 max-w-4xl">
      <div className="space-y-8">
        {/* About Section */}
        <div>
          <h2 className="text-lg font-bold mb-4">About OpenAgents</h2>
          <p className="text-sm mb-4">
            OpenAgents is an Austin-based applied AI lab building the agentic
            economy on open protocols.
          </p>
        </div>

        {/* Vision Section */}
        <div>
          <h2 className="text-lg font-bold mb-4">Our Vision</h2>
          <p className="text-sm mb-4">
            The future of artificial intelligence cannot belong solely to
            monolithic mega-corporations using regulatory capture and incestuous
            relationships with governments to gain power at the expense of the
            people. AI, like bitcoin, can be a liberating technology.
          </p>
        </div>

        {/* Leadership Section */}
        <div>
          <h2 className="text-lg font-bold mb-4">Leadership</h2>
          <div className="border border-white p-6">
            <h3 className="text-base font-semibold mb-3">Christopher David</h3>
            <p className="text-sm mb-4">Founder & CEO</p>
            <p className="text-sm mb-4">
              Christopher has been writing software since the mid-'90s. Bitcoin
              class of '11, Nostr class of '21 and a NIP author, he previously
              founded a successful rideshare startup in Austin. From OpenAI's
              first dev day in late 2023, he saw the potential for AI agents
              built on truly open protocols with bitcoin micropayments.
            </p>
            <p className="text-sm mb-4">
              Since then, he's recorded 150+ videos building in public, shipped
              multiple MVPs, and developed the vision for a decentralized agents
              marketplace running on bitcoin and Nostr. The goal: a clean mobile
              app that doubles as both Nostr client and bitcoin wallet, enabling
              users to easily interact with their own personal AI agent.
            </p>
          </div>
        </div>

        {/* Product Section */}
        <div>
          <h2 className="text-lg font-bold mb-4">Current Focus</h2>
          <p className="text-sm mb-4">
            We're rapidly developing Onyx, our mobile app now in open beta for
            Android and iOS. It's a beautiful mobile application that provides
            access to a decentralized marketplace of agents, where providers are
            incentivized via bitcoin rewards to help build the best agents.
          </p>
          <p className="text-sm mb-2">The app combines:</p>
          <ul className="list-disc ml-6 space-y-2 text-sm">
            <li>Decentralized agents marketplace</li>
            <li>Built-in Bitcoin Lightning wallet</li>
            <li>Nostr client integration</li>
            <li>Personal AI assistant capabilities</li>
          </ul>
        </div>
      </div>
    </div>
  );
}
