import { useEffect } from "react";

/**
 * Ensures the root element carries the `dark` class while mounted.
 */
export function useDarkModeRoot() {
  useEffect(() => {
    document.documentElement.classList.add("dark");
    return () => {
      document.documentElement.classList.remove("dark");
    };
  }, []);
}

