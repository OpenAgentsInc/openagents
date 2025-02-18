import type { Route } from "../+types/cota";

export function meta({}: Route.MetaArgs) {
  return [
    { title: "OpenAgents - Chains of Thought and Action" },
    {
      name: "description",
      content: "Chains of Thought and Action (CoTA) - OpenAgents",
    },
  ];
}

export default function CoTA() {
  return (
    <div className="flex justify-center">
      <article className="my-2 prose w-full text-white">
        <h1 className="text-white">Chains of Thought and Action (CoTA)</h1>
        <p>
          We define a <i>chain of thought and action</i> as a series of
          reasoning steps and tool use whereby agentic AI systems show both the
          intermediate reasoning and the inputs and outputs of actions taken.
        </p>

        <h2 className="text-white">Chain of Thought</h2>
        <p>
          A <i>chain of thought</i> is a series of intermediate reasoning steps
          that significantly improves the ability of large language models to
          perform complex reasoning. Introduced in{" "}
          <a
            href="https://arxiv.org/abs/2201.11903"
            className="text-white hover:opacity-75 underline"
            target="_blank"
          >
            Chain-of-Thought Prompting Elicits Reasoning in Large Language
            Models
          </a>
          , this technique demonstrated a 20-30% improvement in reasoning
          accuracy by having models explain their solutions step-by-step, rather
          than jumping directly to answers.
        </p>

        <p>
          OpenAI has embraced this concept with their O1 series of models, which
          are specifically designed to "spend more time thinking before they
          respond." However, OpenAI has been{" "}
          <a
            href="https://x.com/parshantdeep/status/1834281496850694544"
            className="text-white hover:opacity-75 underline"
            target="_blank"
          >
            criticized
          </a>{" "}
          for not showing the chain of thought to users, unlike competitors like
          DeepSeek which display the complete, unedited reasoning steps.
        </p>

        <h2 className="text-white">And Action</h2>
        <p>
          While chain of thought reveals reasoning, modern AI systems need to do
          more than just think - they need to act. This means using tools:
          calling APIs, reading files, running tests, and making changes to
          systems. Just as we want transparency in reasoning, we need visibility
          into these actions. Each API call, file read, or system change should
          be logged and displayed, creating a complete trace of not just what
          the AI thought, but what it did.
        </p>
        <p>
          This combination of transparent reasoning and visible actions is
          crucial for building trust. When an AI system makes changes to your
          codebase or interacts with your systems, you should be able to see
          both its decision process and a complete record of its actions. This
          allows for both verification of its logic and audit of its activities.
        </p>

        <h2 className="text-white">An Example</h2>
        <p>
          Imagine a 'GitHub issue solver' agent that submits a pull request with
          code that solves a given GitHub issue. It would combine multi-step
          reasoning and tool use to understand codebases and documentation, use
          CI tools and tests, and interact with the GitHub and other external
          APIs.
        </p>
        <p>
          Importantly, this agent would show the full trace of its CoTA:
          including both its reasoning (using a model like DeepSeek R1) and tool
          use (displaying input/output of each tool as sent to the LLM).
        </p>
        <p>The workflow combines multiple models and tools, perhaps:</p>

        <ul className="list-disc list-inside text-white">
          <li>Build repository map from issue context (automated script)</li>
          <li>
            Identify relevant files (DeepSeek R1 for reasoning, Mistral Small
            for structured output)
          </li>
          <li>Traverse and analyze codebase (file readers, AST parsers)</li>
          <li>Plan changes (DeepSeek R1 with full reasoning trace)</li>
          <li>Generate and test code changes (coding tools, CI integration)</li>
          <li>
            Create pull request with detailed explanation (GitHub API tools)
          </li>
        </ul>
        <p>
          At each step, both the reasoning process and tool interactions are
          logged and displayed, creating a complete audit trail of how the
          solution was developed.
        </p>
        <p>
          Entrusting such an agent to make sensitive edits to a codebase may be
          foolish if its creator were to refuse to show you its thought process!
        </p>
        <p>
          We will build the above 'issue solver' agent over the next few videos
          of our{" "}
          <a
            className="text-white hover:opacity-75 underline"
            href="/video-series"
          >
            series
          </a>
          .
        </p>
      </article>
    </div>
  );
}
