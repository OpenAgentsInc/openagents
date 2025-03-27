import React from "react";
import { useMCP } from "@openagents/core";
import { Button, Chat } from "@openagents/ui";

export default function HomePage() {
  return (
    <div className="font-mono flex h-full text-white">
      <Chat />
    </div>
  );
}
