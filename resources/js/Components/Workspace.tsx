import React, { useState } from "react";
import { Shell } from "./tabs/Shell";
import { Browser } from "./tabs/Browser";
import { Editor } from "./tabs/Editor";
import { Planner } from "./tabs/Planner";
import { Codebases } from "./tabs/Codebases";
import { Artifacts } from "./tabs/Artifacts";

const tabs = [
  { id: "shell", label: "Shell" },
  { id: "browser", label: "Browser" },
  { id: "editor", label: "Editor" },
  { id: "planner", label: "Planner" },
  { id: "codebases", label: "Codebases" },
  { id: "artifacts", label: "Artifacts" },
];

const tabComponents = {
  shell: Shell,
  browser: Browser,
  editor: Editor,
  planner: Planner,
  codebases: Codebases,
  artifacts: Artifacts,
};

export function Workspace() {
  const [activeTab, setActiveTab] = useState("shell");

  const ActiveComponent = tabComponents[activeTab];

  return (
    <div className="w-full h-full flex flex-col bg-zinc-900/25 text-white font-sans">
      <div className="p-4 flex-shrink-0">
        <div className="flex space-x-1">
          {tabs.map((tab) => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={`px-3 py-1 text-sm rounded-t-md transition-colors ${
                activeTab === tab.id
                  ? "bg-zinc-800 text-white"
                  : "bg-zinc-900 text-zinc-400 hover:bg-zinc-800 hover:text-white"
              }`}
            >
              {tab.label}
            </button>
          ))}
        </div>
      </div>
      <div className="flex-grow overflow-auto mx-4">
        <div className="bg-zinc-900 h-full rounded-md p-4">
          <ActiveComponent />
        </div>
      </div>
      <div className="flex-shrink-0 p-4 flex justify-between items-center">
        <div className="flex space-x-2">
          <button className="p-1 rounded-full bg-zinc-800 hover:bg-zinc-700">
            <svg
              xmlns="http://www.w3.org/2000/svg"
              className="h-5 w-5"
              viewBox="0 0 20 20"
              fill="currentColor"
            >
              <path
                fillRule="evenodd"
                d="M15.707 15.707a1 1 0 01-1.414 0l-5-5a1 1 0 010-1.414l5-5a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 010 1.414zm-6 0a1 1 0 01-1.414 0l-5-5a1 1 0 010-1.414l5-5a1 1 0 011.414 1.414L5.414 10l4.293 4.293a1 1 0 010 1.414z"
                clipRule="evenodd"
              />
            </svg>
          </button>
          <button className="p-1 rounded-full bg-zinc-800 hover:bg-zinc-700">
            <svg
              xmlns="http://www.w3.org/2000/svg"
              className="h-5 w-5"
              viewBox="0 0 20 20"
              fill="currentColor"
            >
              <path
                fillRule="evenodd"
                d="M10.293 15.707a1 1 0 010-1.414L14.586 10l-4.293-4.293a1 1 0 111.414-1.414l5 5a1 1 0 010 1.414l-5 5a1 1 0 01-1.414 0z"
                clipRule="evenodd"
              />
              <path
                fillRule="evenodd"
                d="M4.293 15.707a1 1 0 010-1.414L8.586 10 4.293 5.707a1 1 0 011.414-1.414l5 5a1 1 0 010 1.414l-5 5a1 1 0 01-1.414 0z"
                clipRule="evenodd"
              />
            </svg>
          </button>
        </div>
      </div>
    </div>
  );
}
