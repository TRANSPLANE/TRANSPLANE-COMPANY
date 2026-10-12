import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "@fontsource/fusion-pixel-12px-proportional-sc/400.css";
import "./styles.css";
import "./pixel-game.css";
import "./narrative-game.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
