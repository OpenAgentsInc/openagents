import { Button } from "@openagents/ui";
import React, { useEffect } from "react";
import { Ionicons } from "@expo/vector-icons";
import loadIconFonts from "../shims/load-icon-fonts";

export default function HomePage() {
  // Load icon fonts on component mount
  useEffect(() => {
    loadIconFonts();
  }, []);
  // Function to render Ionicons
  const renderIcon = (iconName: string) => {
    return <Ionicons name={iconName as any} size={20} color="#ffffff" />;
  };

  return (
    <div className="flex h-full flex-col items-center justify-center gap-4">
      <Button label="Normal Button" variant="primary" />

      <Button
        label="Icon Button"
        variant="secondary"
        leftIcon="heart"
        renderIcon={renderIcon}
      />

      <Button
        label="Settisdsdngs"
        variant="primary"
        leftIcon="settings-outline"
        renderIcon={renderIcon}
      />
    </div>
  );
}
