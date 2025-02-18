import type { Route } from "../+types/services";

export function meta({}: Route.MetaArgs) {
  return [
    { title: "OpenAgents - Services" },
    { name: "description", content: "OpenAgents Business Services" },
  ];
}

export default function Services() {
  return (
    <div className="container mx-auto px-4 max-w-4xl">
      <div className="space-y-4">
        <h1 className="text-lg font-bold mb-4">Business Pro</h1>
        <p className="text-sm">We offer managed coding agent services.</p>
        <p className="text-sm">
          They're best for small businesses that need software development and
          don't want the hassle of hiring new developers.
        </p>
        <p className="text-sm">
          Describe the software you want, we supervise our agents to build it.
        </p>
        <p className="text-sm">
          Plans start at $500/month. Limited availability.
        </p>
        <p className="text-sm">
          <a
            href="https://pay.openagents.com/b/6oEeW91rx3XG5K89AE"
            target="_blank"
            className="text-white hover:text-white/80 underline"
          >
            Sign up now
          </a>
          {" or "}
          <a
            href="https://calendly.com/christopher-david-openagents/custom-agent-consultation"
            className="text-white hover:text-white/80 underline"
          >
            set up a call
          </a>
          {" to discuss."}
        </p>
      </div>
    </div>
  );
}
