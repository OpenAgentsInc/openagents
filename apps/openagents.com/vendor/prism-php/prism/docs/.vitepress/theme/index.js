import { h } from "vue";
import DefaultTheme from "vitepress/theme";
import HostedFooter from "./HostedFooter.vue";
import "./custom.css";

export default {
  extends: DefaultTheme,
  Layout() {
    return h(DefaultTheme.Layout, null, {
      "layout-bottom": () => h(HostedFooter),
    });
  },
};
