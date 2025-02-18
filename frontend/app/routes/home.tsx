import type { Route, BlogPost } from "../+types/home";
import ReactMarkdown from "react-markdown"
import { BLOG_POSTS } from "../+types/home"

export function meta({ }: Route.MetaArgs) {
  return [
    { title: "OpenAgents - Home" },
    { name: "description", content: "Welcome to OpenAgents" },
  ];
}

function BlogPost({ date, title, content }: BlogPost) {
  return (
    <div className="pb-4">
      <div className="flex items-center gap-4 mb-2">
        <h3 className="text-base font-semibold">{title}</h3>
        <p className="text-white/50 text-sm">{date}</p>
      </div>
      <div className="text-sm prose prose-invert prose-a:text-white prose-a:underline hover:prose-a:text-white/80">
        <ReactMarkdown>{content}</ReactMarkdown>
      </div>
    </div>
  );
}

export default function Home() {
  return (
    <div className="container mx-auto px-4 max-w-4xl">
      <div className="grid grid-cols-1 md:grid-cols-2 gap-8 mt-4 mb-8">
        {/* About Column */}
        <div>
          <h2 className="text-lg font-bold mb-4">About OpenAgents</h2>
          <div className="space-y-4">
            <p className="text-sm">
              OpenAgents is an applied AI lab writing software for the agentic
              economy on open protocols.
            </p>
            <p className="text-sm">
              Our flagship product is Onyx, a mobile app equipping every person with
              their own personal AI agent.
            </p>
            <p className="text-sm">
              Onyx will be your gateway to an open marketplace of the best AI
              agents.
            </p>
            <p className="text-sm">
              <a href="/onyx" className="text-white/80 hover:text-white underline">
                Download Onyx in beta for Android & iOS now.
              </a>
            </p>
          </div>
        </div>

        {/* Principles Column */}
        <div>
          <h2 className="text-lg font-bold mb-4">Our Principles</h2>
          <ul className="list-disc ml-6 space-y-2 text-sm">
            <li>
              Open Source <span className="text-white/50">[</span>
              <a
                href="https://github.com/OpenAgentsInc"
                target="_blank"
                className="text-white/50 hover:text-white/80 underline"
              >
                GitHub
              </a>
              <span className="text-white/50">]</span>
            </li>
            <li>
              Build in Public <span className="text-white/50">[</span>
              <a
                href="/video-series"
                className="text-white/50 hover:text-white/80 underline"
              >
                Video Series
              </a>
              <span className="text-white/50">]</span>
            </li>
            <li>Bitcoin Only</li>
            <li>
              One Market <span className="text-white/50">[</span>
              <a
                href="https://x.com/OpenAgentsInc/status/1866351898376405220"
                target="_blank"
                className="text-white/50 hover:text-white/80 underline"
              >
                Video
              </a>
              <span className="text-white/50">]</span>
            </li>
            <li>
              Neutrality Wins <span className="text-white/50">[</span>
              <a
                href="https://x.com/OpenAgentsInc/status/1870373254197613052"
                target="_blank"
                className="text-white/50 hover:text-white/80 underline"
              >
                Video
              </a>
              <span className="text-white/50">]</span>
            </li>
            <li>
              Pay Contributors Rev-Share <span className="text-white/50">[</span>
              <a
                href="https://github.com/OpenAgentsInc/openagents/wiki/Flow-of-Funds"
                target="_blank"
                className="text-white/50 hover:text-white/80 underline"
              >
                Spec
              </a>
              <span className="text-white/50">]</span>
            </li>
            <li>Incentivized Interoperability</li>
          </ul>
        </div>
      </div>

      {/* Blog Section */}
      <div className="border border-white p-6">
        <h2 className="text-lg font-bold mb-6">Latest Updates</h2>
        {BLOG_POSTS.map((post, index) => (
          <BlogPost key={index} {...post} />
        ))}
      </div>
    </div>
  );
}
