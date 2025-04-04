import { exposeThemeContext } from "./theme/theme-context";
import { exposeWindowContext } from "./window/window-context";
import { exposeCommandContext } from "./command/command-preload";
import { exposeApiPortContext } from "./api-port/api-port-context";
import { exposeDbStatusContext } from "./db-status/db-status-context"; // Import new context exposer

export default function exposeContexts() {
  console.log('[Preload] Exposing contexts...');
  exposeWindowContext();
  exposeThemeContext();
  exposeCommandContext();
  exposeApiPortContext();
  exposeDbStatusContext(); // Expose the new context
  console.log('[Preload] Contexts exposed.');
}
