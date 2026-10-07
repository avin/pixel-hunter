import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "@/components/app/App/App";
import {
  isScreenOverlayView,
  ScreenOverlayApp,
} from "@/components/app/ScreenOverlayApp/ScreenOverlayApp";
import { store } from "@/store";
import "./index.css";

const search = window.location.search;
document.documentElement.dataset.view = new URLSearchParams(search).get("view") ?? "main";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {isScreenOverlayView(search) ? (
      <ScreenOverlayApp store={store} search={search} />
    ) : (
      <App store={store} />
    )}
  </React.StrictMode>,
);
