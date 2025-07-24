import React from "react";
import { usePaneStore } from "@/stores/pane";
import { Pane } from "./Pane";
import { Pane as PaneType } from "@/types/pane";
import { ChatPane } from "./ChatPane";
import { MetadataPane } from "./MetadataPane";

export const PaneManager: React.FC = () => {
  const { panes, activePaneId } = usePaneStore();

  // Get data from global object (temporary solution)
  const data = (window as any).__openagents_data || {};

  const renderPaneContent = (pane: PaneType) => {
    switch (pane.type) {
      case "chat":
        const sessionId = pane.content?.sessionId as string;
        const session = data.sessions?.find((s: any) => s.id === sessionId);
        return <ChatPane 
          pane={pane} 
          session={session}
          sendMessage={data.sendMessage}
          updateSessionInput={data.updateSessionInput}
        />;
      case "metadata":
        return <MetadataPane 
          claudeStatus={data.claudeStatus}
          sessions={data.sessions}
          newProjectPath={data.newProjectPath}
          isDiscoveryLoading={data.isDiscoveryLoading}
          setNewProjectPath={data.setNewProjectPath}
          createSession={data.createSession}
          stopSession={data.stopSession}
        />;
      default:
        return <div>Unknown pane type: {pane.type}</div>;
    }
  };

  const baseZIndex = 1000;

  return (
    <>
      {panes.map((pane, index) => (
        <Pane
          key={pane.id}
          {...pane}
          isActive={pane.id === activePaneId}
          style={{
            zIndex: baseZIndex + index,
          }}
        >
          {renderPaneContent(pane)}
        </Pane>
      ))}
    </>
  );
};