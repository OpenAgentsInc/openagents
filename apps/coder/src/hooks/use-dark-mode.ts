import { useEffect, useState } from "react";
import { getCurrentTheme, setTheme, ThemePreferences } from "../helpers/theme_helpers";
import { ThemeMode } from "../types/theme-mode";

export function useDarkMode() {
  const [themePrefs, setThemePrefs] = useState<ThemePreferences>({ 
    system: "system", 
    local: null 
  });
  
  // Track if dark mode is currently active
  const [isDark, setIsDark] = useState(false);

  // Toggle theme function
  const toggleDarkMode = async () => {
    const newTheme = isDark ? "light" : "dark";
    await setTheme(newTheme);
    
    // Update state after theme changes
    const updatedPrefs = await getCurrentTheme();
    setThemePrefs(updatedPrefs);
    
    // Check if document actually has dark class
    const hasDarkClass = document.documentElement.classList.contains("dark");
    setIsDark(hasDarkClass);
    
    return hasDarkClass;
  };

  // Set specific theme
  const changeTheme = async (theme: ThemeMode) => {
    await setTheme(theme);
    
    // Update state after theme changes
    const updatedPrefs = await getCurrentTheme();
    setThemePrefs(updatedPrefs);
    
    // Check if document actually has dark class
    const hasDarkClass = document.documentElement.classList.contains("dark");
    setIsDark(hasDarkClass);
    
    return hasDarkClass;
  };

  // Sync theme on mount
  useEffect(() => {
    const syncTheme = async () => {
      const currentPrefs = await getCurrentTheme();
      setThemePrefs(currentPrefs);
      
      // Check if document actually has dark class
      const hasDarkClass = document.documentElement.classList.contains("dark");
      setIsDark(hasDarkClass);
    };

    syncTheme();

    // Also observe the dark class on the html element
    const observer = new MutationObserver((mutations) => {
      mutations.forEach((mutation) => {
        if (
          mutation.type === 'attributes' && 
          mutation.attributeName === 'class'
        ) {
          const hasDarkClass = document.documentElement.classList.contains("dark");
          setIsDark(hasDarkClass);
        }
      });
    });

    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['class']
    });

    return () => observer.disconnect();
  }, []);

  return {
    isDark,
    theme: themePrefs.local || themePrefs.system,
    themePrefs,
    toggleDarkMode,
    changeTheme,
  };
}