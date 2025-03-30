import React from "react";
import { SiElectron, SiReact, SiVite } from "@icons-pack/react-simple-icons";
import { createReactComponent } from "@openagents/core/src/utils/reactCompatibility";

// Create React 19 compatible versions of the icons
const ReactIcon = createReactComponent(SiReact as React.ComponentType<any>);
const ViteIcon = createReactComponent(SiVite as React.ComponentType<any>);
const ElectronIcon = createReactComponent(SiElectron as React.ComponentType<any>);

export default function InitalIcons() {
  const iconSize = 48;

  return (
    <div className="inline-flex gap-2">
      <ReactIcon size={iconSize} />
      <ViteIcon size={iconSize} />
      <ElectronIcon size={iconSize} />
    </div>
  );
}
