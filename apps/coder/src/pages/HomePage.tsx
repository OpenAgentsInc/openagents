import { Button } from "@openagents/ui";
import { useMCP } from "@openagents/core"
import React, { useState } from "react";

export default function HomePage() {
  const { status, result, error, serverUrl, callTool } = useMCP();
  const [num1, setNum1] = useState<number>(0);
  const [num2, setNum2] = useState<number>(0);

  return (
    <div className="font-mono flex h-full flex-col items-center justify-center gap-4 text-white">
      <div className="mb-4 text-center">
        <p>MCP Status: {status}</p>
        <p className="text-sm text-gray-400 mt-1">{serverUrl}</p>
        {result && <p className="mt-2">Result: {result}</p>}
        {error && <p className="text-red-500">Error: {error.message}</p>}
      </div>

      <div className="flex flex-col gap-4 items-center">
        <div className="flex gap-4 items-center">
          <input
            type="number"
            value={num1}
            onChange={(e) => setNum1(Number(e.target.value))}
            className="bg-black border border-white rounded px-3 py-2 w-24 text-white"
            placeholder="First number"
          />
          <span className="text-2xl">+</span>
          <input
            type="number"
            value={num2}
            onChange={(e) => setNum2(Number(e.target.value))}
            className="bg-black border border-white rounded px-3 py-2 w-24 text-white"
            placeholder="Second number"
          />
        </div>

        <Button
          label="Calculate"
          variant="primary"
          onPress={() => callTool(num1, num2)}
        />
      </div>
    </div>
  );
}
