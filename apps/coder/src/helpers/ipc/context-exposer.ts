import { exposeThemeContext } from "./theme/theme-context";
import { exposeWindowContext } from "./window/window-context";
import { exposeCommandContext } from "./command/command-preload";
import { exposeFetchContext } from "./fetch/fetch-context";

export default function exposeContexts() {
  exposeWindowContext();
  exposeThemeContext();
  exposeCommandContext();
  exposeFetchContext();
}
