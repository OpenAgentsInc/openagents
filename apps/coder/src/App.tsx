import React, { useEffect } from "react";
import { createRoot } from "react-dom/client";
import { syncThemeWithLocal } from "./helpers/theme_helpers";
import { useTranslation } from "react-i18next";
import "./localization/i18n";
import { updateAppLanguage } from "./helpers/language_helpers";
import { router } from "./routes/router";
import { RouterProvider } from "@tanstack/react-router";

// Immediately sync theme before any rendering
// But also add this to a useEffect to ensure it runs after hydration
(async () => {
  try {
    await syncThemeWithLocal();
    console.log("Theme synchronized on initial load");
  } catch (error) {
    console.error("Failed to sync theme:", error);
  }
})();

export default function App() {
  const { i18n } = useTranslation();

  // Sync theme and language after component mounts
  useEffect(() => {
    // Sync theme again after React hydration is complete
    syncThemeWithLocal().then(() => {
      console.log("Theme synchronized after component mount");
    });
    
    // Update app language
    updateAppLanguage(i18n);
  }, [i18n]);

  return <RouterProvider router={router} />;
}

const root = createRoot(document.getElementById("app")!);
root.render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
