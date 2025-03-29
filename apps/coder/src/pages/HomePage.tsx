import React, { useState } from "react";
import { CommandProvider } from "../helpers/ipc/command/CommandProvider";
import { ChatWithCommandSupport } from "../components/ChatWithCommandSupport";
import { AgentChatTest } from "../components/AgentChatTest";

export default function HomePage() {
  const [showAgentTest, setShowAgentTest] = useState(false);
  
  return (
    <div className="font-mono flex flex-col h-full text-white">
      <div className="flex flex-row justify-between items-center p-2 bg-gray-800">
        <div className="text-lg font-bold">Coder</div>
        <button 
          className="px-4 py-2 bg-blue-600 rounded hover:bg-blue-700 transition-colors"
          onClick={() => setShowAgentTest(!showAgentTest)}
        >
          {showAgentTest ? 'Standard Chat' : 'Agent Test'}
        </button>
      </div>
      
      <div className="flex-1 overflow-hidden flex flex-col">
        <CommandProvider>
          {showAgentTest ? (
            <div className="h-full flex-1 flex bg-black">
              <AgentChatTest />
            </div>
          ) : (
            <div className="flex-1 flex flex-col">
              <ChatWithCommandSupport />
            </div>
          )}
        </CommandProvider>
      </div>
    </div>
  );
}
