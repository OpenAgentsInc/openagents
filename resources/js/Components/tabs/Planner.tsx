import React from "react";
import { useMessageStore } from "../../store";

export function Planner() {
  const { currentPlan } = useMessageStore();

  const parseMarkdown = (markdown) => {
    const lines = markdown.split("\n");
    const parsedLines = lines.map((line) => {
      if (line.startsWith("-[x]")) {
        return {
          type: "checkbox",
          checked: true,
          content: line.slice(4).trim(),
        };
      } else if (line.startsWith("-[ ]")) {
        return {
          type: "checkbox",
          checked: false,
          content: line.slice(4).trim(),
        };
      } else if (line.trim() === "") {
        return { type: "empty" };
      } else {
        return { type: "text", content: line.trim() };
      }
    });
    return parsedLines;
  };

  const renderLine = (line, index) => {
    switch (line.type) {
      case "checkbox":
        return (
          <li key={index} className="flex items-center mb-2">
            <input
              type="checkbox"
              checked={line.checked}
              readOnly
              className="mr-2"
            />
            <span>{line.content}</span>
          </li>
        );
      case "text":
        return (
          <p key={index} className="my-0">
            {line.content}
          </p>
        );
      case "empty":
        return <br key={index} />;
      default:
        return null;
    }
  };

  const parsedPlan = parseMarkdown(currentPlan);

  return (
    <div className="text-white">
      <h2 className="text-2xl font-bold mb-4">Planner</h2>
      <div className="bg-zinc-800 rounded-md p-4 prose prose-invert max-w-none">
        <ul className="-mt-8 list-none pl-0">
          {parsedPlan.map((line, index) => renderLine(line, index))}
        </ul>
      </div>
    </div>
  );
}
