import { ThemeMode } from "@/types/theme-mode";

const THEME_KEY = "theme";

export interface ThemePreferences {
  system: ThemeMode;
  local: ThemeMode | null;
}

export async function getCurrentTheme(): Promise<ThemePreferences> {
  const currentTheme = await window.themeMode.current();
  const localTheme = localStorage.getItem(THEME_KEY) as ThemeMode | null;

  return {
    system: currentTheme,
    local: localTheme,
  };
}

export async function setTheme(newTheme: ThemeMode) {
  switch (newTheme) {
    case "dark":
      await window.themeMode.dark();
      updateDocumentTheme(true);
      break;
    case "light":
      await window.themeMode.light();
      updateDocumentTheme(false);
      break;
    case "system": {
      const isDarkMode = await window.themeMode.system();
      updateDocumentTheme(isDarkMode);
      break;
    }
  }

  localStorage.setItem(THEME_KEY, newTheme);
}

export async function toggleTheme() {
  try {
    // Toggle the theme in electron
    const isDarkMode = await window.themeMode.toggle();
    const newTheme = isDarkMode ? "dark" : "light";
    
    // Update DOM and localStorage
    updateDocumentTheme(isDarkMode);
    localStorage.setItem(THEME_KEY, newTheme);
    
    // Force a style recalculation
    document.body.offsetHeight;
  } catch (error) {
    console.error("Error toggling theme:", error);
  }
}

export async function syncThemeWithLocal() {
  const { local } = await getCurrentTheme();
  if (!local) {
    setTheme("system");
    return;
  }

  await setTheme(local);
}

function updateDocumentTheme(isDarkMode: boolean) {
  // First, apply theme to html element (root)
  if (!isDarkMode) {
    document.documentElement.classList.remove("dark");
    document.documentElement.style.colorScheme = "light";
  } else {
    document.documentElement.classList.add("dark");
    document.documentElement.style.colorScheme = "dark";
  }
  
  // Force CSS to repaint by adding a trivial style change and removing it
  document.documentElement.style.setProperty("--repaint-hack", "1");
  setTimeout(() => {
    document.documentElement.style.removeProperty("--repaint-hack");
  }, 0);
}
