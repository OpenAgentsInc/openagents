import React, { useState, useCallback } from "react";
import { Tabs } from "@shopify/polaris";
import { Shell } from "./tabs/Shell";
import { Browser } from "./tabs/Browser";
import { Editor } from "./tabs/Editor";
import { Planner } from "./tabs/Planner";
import { Codebases } from "./tabs/Codebases";
import { Artifacts } from "./tabs/Artifacts";

export function Workspace() {
  const [selected, setSelected] = useState(0);

  const handleTabChange = useCallback(
    (selectedTabIndex: number) => setSelected(selectedTabIndex),
    []
  );

  const tabs = [
    {
      id: "shell",
      content: "Shell",
      accessibilityLabel: "Shell tab",
      panelID: "shell-content",
    },
    {
      id: "browser",
      content: "Browser",
      accessibilityLabel: "Browser tab",
      panelID: "browser-content",
    },
    {
      id: "editor",
      content: "Editor",
      accessibilityLabel: "Editor tab",
      panelID: "editor-content",
    },
    {
      id: "planner",
      content: "Planner",
      accessibilityLabel: "Planner tab",
      panelID: "planner-content",
    },
    {
      id: "codebases",
      content: "Codebases",
      accessibilityLabel: "Codebases tab",
      panelID: "codebases-content",
    },
    {
      id: "artifacts",
      content: "Artifacts",
      accessibilityLabel: "Artifacts tab",
      panelID: "artifacts-content",
    },
  ];

  const tabComponents = [
    <Shell />,
    <Browser />,
    <Editor />,
    <Planner />,
    <Codebases />,
    <Artifacts />,
  ];

  return (
    <div className="pt-8 w-1/2 min-w-[650px] min-h-full bg-bg-100 border-l border-zinc-800">
      <Tabs tabs={tabs} selected={selected} onSelect={handleTabChange}>
        <div className="p-4 h-full">{tabComponents[selected]}</div>
      </Tabs>
    </div>
  );
}
