import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./app";
import "./styles.css";

const root = document.getElementById("root");

if (!root) {
  throw new Error("The renderer root is unavailable.");
}

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
