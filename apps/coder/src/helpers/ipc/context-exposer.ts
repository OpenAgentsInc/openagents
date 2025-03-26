import { exposeThemeContext } from "./theme/theme-context";
import { exposeWindowContext } from "./window/window-context";
import { exposeMcpContext } from "./mcp/mcp-context";

export default function exposeContexts() {
  exposeWindowContext();
  exposeThemeContext();
  exposeMcpContext();
}
