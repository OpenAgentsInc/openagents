import "./bootstrap";
import "../css/app.css";
import "@shopify/polaris/build/esm/styles.css";

import { AppProvider } from "@shopify/polaris";
import { createRoot } from "react-dom/client";
import { createInertiaApp } from "@inertiajs/react";
import { resolvePageComponent } from "laravel-vite-plugin/inertia-helpers";

const appName = import.meta.env.VITE_APP_NAME || "OpenAgents";

createInertiaApp({
  title: (title) => "OpenAgents",
  resolve: (name) =>
    resolvePageComponent(
      `./Pages/${name}.tsx`,
      import.meta.glob("./Pages/**/*.tsx")
    ),
  setup({ el, App, props }) {
    const root = createRoot(el);
    root.render(
      <div className="fixed inset-0 overflow-hidden bg-black">
        <AppProvider i18n={{}}>
          <App {...props} />
        </AppProvider>
      </div>
    );
    document.body.classList.add("overflow-hidden", "h-full");
  },
  progress: {
    color: "#4B5563",
  },
});
