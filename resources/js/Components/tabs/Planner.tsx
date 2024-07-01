import React from "react";
import { useMessageStore } from "../../store";
import ReactMarkdown from "react-markdown";

export function Planner() {
  const currentPlan = useMessageStore((state) => state.currentPlan);

  return (
    <div className="text-white">
      <h2 className="text-2xl font-bold mb-4">Planner</h2>
      <div className="bg-zinc-800 rounded-md p-4 prose prose-invert max-w-none">
        <ReactMarkdown>{currentPlan}</ReactMarkdown>
      </div>
    </div>
  );
}
