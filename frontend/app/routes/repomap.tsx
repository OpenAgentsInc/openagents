import { useState } from "react"

import type { Route, RepoMapResponse } from "../+types/repomap";

export function meta({ }: Route.MetaArgs) {
  return [
    { title: "OpenAgents - Repository Map Generator" },
    { name: "description", content: "Generate a detailed map of any GitHub repository's structure" },
  ];
}

export default function RepoMap() {
  const [loading, setLoading] = useState(false);
  const [repoMap, setRepoMap] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    const formData = new FormData(e.currentTarget);
    const repoUrl = formData.get("repo_url") as string;

    setLoading(true);
    try {
      const response = await fetch("/repomap/generate", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({ repo_url: repoUrl }),
      });
      const data: RepoMapResponse = await response.json();
      setRepoMap(data.repo_map);
    } catch (error) {
      console.error("Error generating repo map:", error);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="space-y-8">
      <div className="space-y-4 text-sm">
        <h1 className="text-lg font-bold mb-4">Repository Map Generator</h1>
        <p className="text-sm mb-4">
          Generate a detailed map of any GitHub repository's structure and
          relationships. This tool uses{" "}
          <a
            href="https://aider.chat"
            target="_blank"
            className="font-bold underline"
          >
            Aider's
          </a>{" "}
          repository mapping technology to create a concise overview that helps AI
          agents and developers understand codebases more effectively.
        </p>
        <p className="text-sm mb-4">
          The map includes key classes, functions, and their relationships,
          optimized to fit within LLM context windows while preserving the most
          important details about the codebase structure.
        </p>
        <p className="mb-4">
          Read{" "}
          <a
            href="https://aider.chat/docs/repomap.html"
            target="_blank"
            className="font-bold underline"
          >
            the Aider repomap documentation
          </a>{" "}
          for more info.
        </p>
      </div>

      <div className="space-y-6">
        {/* Repository URL Input Form */}
        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label htmlFor="repo_url" className="block text-sm font-medium">
              GitHub Repository URL
            </label>
            <input
              type="text"
              name="repo_url"
              id="repo_url"
              placeholder="https://github.com/username/repo"
              className="mt-1 block w-[600px] border border-white/50 bg-black px-3 py-2 text-white placeholder-white/50 focus:border-white focus:outline-none focus:ring-1 focus:ring-white text-sm"
              required
            />
          </div>

          <button
            type="submit"
            className="inline-flex items-center justify-center border border-white bg-black px-4 py-2 text-sm font-medium text-white hover:bg-white hover:text-black focus:outline-none focus:ring-2 focus:ring-white focus:ring-offset-2 disabled:opacity-50 disabled:cursor-not-allowed"
            disabled={loading}
          >
            {loading ? "Generating..." : "Generate Map"}
          </button>
        </form>

        {/* Loading Indicator */}
        {loading && (
          <div className="flex items-center justify-center py-4">
            <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-white"></div>
            <span className="ml-2 text-gray-300">Generating repository map...</span>
          </div>
        )}

        {/* Results Area */}
        {repoMap && (
          <div className="text-xs max-w-none">
            <pre><code>{repoMap}</code></pre>
          </div>
        )}
      </div>
    </div>
  );
}
