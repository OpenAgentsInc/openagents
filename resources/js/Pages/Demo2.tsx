import React from "react";
import { Loader2, HelpCircle, Key, LogIn } from "lucide-react";

const AutoDevWorkspace = () => {
  return (
    <div className="flex h-screen bg-black text-white font-mono">
      {/* Left Sidebar */}
      <div className="w-64 border-r border-gray-800 p-4 flex flex-col">
        <div className="flex items-center mb-8">
          <div className="w-8 h-8 bg-blue-500 rounded-full mr-2"></div>
          <span className="text-xl font-semibold">AutoDev</span>
        </div>
        <div className="flex-grow"></div>
        <div className="space-y-4">
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

      {/* Main Content */}
      <div className="flex-grow flex">
        {/* Middle Section */}
        <div className="flex-grow flex items-center justify-center">
          <div className="text-center">
            <Loader2 className="w-12 h-12 animate-spin mx-auto mb-4" />
            <p>Loading...</p>
          </div>
        </div>

        {/* Right Sidebar */}
        <div className="w-80 border-l border-gray-800 p-4">
          <div className="mb-4">
            <h2 className="text-xl font-semibold mb-2">AutoDev's Workspace</h2>
            <div className="flex items-center justify-between">
              <span>Following</span>
              <div className="w-12 h-6 bg-blue-500 rounded-full"></div>
            </div>
          </div>
          <div className="flex space-x-2 mb-4">
            {["Shell", "Browser", "Editor", "Planner"].map((tab) => (
              <button
                key={tab}
                className="px-3 py-1 text-sm bg-gray-900 rounded"
              >
                {tab}
              </button>
            ))}
          </div>
          <div className="flex-grow bg-gray-900 rounded-lg p-4 mb-4"></div>
          <div className="flex justify-start items-center">
            <div className="flex space-x-2">
              <button className="p-2 bg-gray-900 rounded">
                <svg
                  className="w-4 h-4"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                >
                  <polyline points="11 17 6 12 11 7"></polyline>
                </svg>
              </button>
              <button className="p-2 bg-gray-900 rounded">
                <svg
                  className="w-4 h-4"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                >
                  <polyline points="13 17 18 12 13 7"></polyline>
                </svg>
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

export default AutoDevWorkspace;
