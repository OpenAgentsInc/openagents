import React from "react";
import { HelpCircle, Key, LogIn } from "lucide-react";

const AutoDevWorkspace = () => {
  return (
    <div className="flex h-screen bg-black text-white font-mono">
      {/* Left Sidebar - Thread List */}
      <div className="w-64 border-r border-gray-800 p-4 flex flex-col">
        <div className="flex items-center mb-8">
          <div className="w-8 h-8 bg-blue-500 rounded-full mr-2"></div>
          <span className="text-xl font-semibold">AutoDev</span>
        </div>
        <h3 className="text-sm font-semibold mb-2">Thread List</h3>
        <div className="flex-grow overflow-y-auto">
          {/* Add thread items here */}
        </div>
        <div className="space-y-4 mt-4">
          <button className="flex items-center text-sm">
            <HelpCircle className="w-5 h-5 mr-2" />
            About us
          </button>
          <button className="flex items-center text-sm">
            <Key className="w-5 h-5 mr-2" />
            Request access
          </button>
          <button className="flex items-center text-sm">
            <LogIn className="w-5 h-5 mr-2" />
            Login
          </button>
        </div>
      </div>

      {/* Main Content - Message List */}
      <div className="flex-1 flex flex-col border-r border-gray-800">
        <div className="p-4 border-b border-gray-800">
          <h2 className="text-xl font-semibold">Message List</h2>
        </div>
        <div className="flex-grow overflow-y-auto p-4">
          {/* Add messages here */}
        </div>
        <div className="p-4 border-t border-gray-800">
          <input
            type="text"
            placeholder="Type your message..."
            className="w-full bg-gray-900 text-white px-4 py-2 rounded"
          />
        </div>
      </div>

      {/* Right Column - Iframe */}
      <div className="flex-1">
        <iframe
          src="https://wanix.openagents.com"
          className="w-full h-full border-none"
          title="Wanix OpenAgents"
        ></iframe>
      </div>
    </div>
  );
};

export default AutoDevWorkspace;
