import { Button } from "@openagents/ui";
import { useMCP } from "@openagents/core"
import React, { useEffect } from "react";
import { Ionicons } from "@expo/vector-icons";
import loadIconFonts from "../shims/load-icon-fonts";

export default function HomePage() {
  const { status, result, error } = useMCP();

  // Load icon fonts on component mount
  useEffect(() => {
    loadIconFonts();
  }, []);

  const renderIcon = (iconName: string) => {
    return <Ionicons name={iconName as any} size={20} color="#ffffff" />;
  };

  return (
    <div className="flex h-full flex-col items-center justify-center gap-4 text-white">
      <div className="mb-4">
        <p>MCP Status: {status}</p>
        {result && <p>Add Tool Result: {result}</p>}
        {error && <p className="text-red-500">Error: {error.message}</p>}
      </div>

      <Button label="Normal Button" variant="primary" />

      <Button
        label="Icon Button"
        variant="secondary"
        leftIcon="heart"
        renderIcon={renderIcon}
      />

      <Button
        label="Settings"
        variant="primary"
        leftIcon="settings-outline"
        renderIcon={renderIcon}
      />
    </div>
  );
}
