import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles/index.css";

// No browser menu on a right click, except in text fields (cut, copy, paste).
window.addEventListener("contextmenu", (e) => {
  const el = e.target as HTMLElement | null;
  if (!el?.closest("input, textarea")) e.preventDefault();
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
