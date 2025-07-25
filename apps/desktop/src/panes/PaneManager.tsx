import React from "react";
import { usePaneStore } from "@/stores/pane";
import { Pane } from "./Pane";
import { Pane as PaneType } from "@/types/pane";
import { ChatPane } from "./ChatPane";
import { HistoryPane } from "./HistoryPane";
import { SettingsPane } from "./SettingsPane";
import { StatsPane } from "./StatsPane";

export const PaneManager: React.FC = () => {
  const { panes, activePaneId } = usePaneStore();

  // Get data from global object (temporary solution)
  const data = (window as any).__openagents_data || {};

  const renderPaneContent = (pane: PaneType) => {
    switch (pane.type) {
      case "chat":
        const sessionId = pane.content?.sessionId as string;
        const session = data.sessions?.find((s: any) => s.id === sessionId);
        console.log('🎨 [PANE-MANAGER] Rendering ChatPane:', {
          paneId: pane.id,
          sessionId,
          sessionFound: !!session,
          totalSessions: data.sessions?.length || 0
        });
        return <ChatPane 
          pane={pane} 
          session={session}
          sendMessage={data.sendMessage}
        />;
      case "metadata":
        return <HistoryPane 
          sessions={data.sessions}
          newProjectPath={data.newProjectPath}
          isDiscoveryLoading={data.isDiscoveryLoading}
          setNewProjectPath={data.setNewProjectPath}
          createSession={data.createSession}
          stopSession={data.stopSession}
        />;
      case "settings":
        return <SettingsPane 
          claudeStatus={data.claudeStatus}
          sessions={data.sessions}
          isDiscoveryLoading={data.isDiscoveryLoading}
        />;
      case "stats":
        return <StatsPane />;
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