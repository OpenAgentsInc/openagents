import React, { useEffect } from "react";
import { syncThemeWithLocal } from "./helpers/theme_helpers";
import { useTranslation } from "react-i18next";
import "./localization/i18n";
import { updateAppLanguage } from "./helpers/language_helpers";
import { router } from "./routes/router";
import { RouterProvider } from "@tanstack/react-router";
import { Toaster } from "./components/ui/sonner";
import { DatabaseErrorProvider } from "./providers/DatabaseErrorProvider";

// Immediately sync theme before any rendering
(async () => {
  try {
    await syncThemeWithLocal();
  } catch (error) {
    console.error("Failed to sync theme:", error);
  }
})();

export default function App() {
  const { i18n } = useTranslation();

  // Sync theme and language after component mounts
  useEffect(() => {
    // Sync theme again after React hydration is complete
    syncThemeWithLocal();
    
    // Update app language
    updateAppLanguage(i18n);
  }, [i18n]);

  return (
    <DatabaseErrorProvider>
      <RouterProvider router={router} />
      <Toaster position="top-right" closeButton />
    </DatabaseErrorProvider>
  );
}