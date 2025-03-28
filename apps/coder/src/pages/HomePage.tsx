import React from "react";
import { CommandProvider } from "../helpers/ipc/command/CommandProvider";
import { ChatWithCommandSupport } from "../components/ChatWithCommandSupport";

export default function HomePage() {
  return (
    <div className="font-mono flex h-full text-white">
      <CommandProvider>
        <ChatWithCommandSupport />
      </CommandProvider>
    </div>
  );
}
