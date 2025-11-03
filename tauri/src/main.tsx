import React from "react";
import ReactDOM from "react-dom/client";
import "./index.css";
import HelloDesktop from "./desktop/HelloDesktop";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <HelloDesktop />
  </React.StrictMode>,
);
