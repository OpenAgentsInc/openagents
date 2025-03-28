import { exposeThemeContext } from "./theme/theme-context";
import { exposeWindowContext } from "./window/window-context";
import { exposeCommandContext } from "./command/command-preload";

export default function exposeContexts() {
  exposeWindowContext();
  exposeThemeContext();
  exposeCommandContext();
}
