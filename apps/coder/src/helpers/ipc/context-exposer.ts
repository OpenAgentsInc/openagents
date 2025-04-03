import { exposeThemeContext } from "./theme/theme-context";
import { exposeWindowContext } from "./window/window-context";
import { exposeCommandContext } from "./command/command-preload";
import { exposeApiPortContext } from "./api-port/api-port-context";

export default function exposeContexts() {
  exposeWindowContext();
  exposeThemeContext();
  exposeCommandContext();
  exposeApiPortContext();
}
