import React from "react";
import { usePaneStore } from "@/stores/pane";
import { Pane } from "./Pane";
import { Pane as PaneType } from "@/types/pane";
import { ChatPane } from "./ChatPane";
import { MetadataPane } from "./MetadataPane";

export const PaneManager: React.FC = () => {
  const { panes, activePaneId } = usePaneStore();

  const renderPaneContent = (pane: PaneType) => {
    switch (pane.type) {
      case "chat":
        return <ChatPane pane={pane} />;
      case "metadata":
        return <MetadataPane />;
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