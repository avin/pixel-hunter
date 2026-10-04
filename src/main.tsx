import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "@/components/app/App/App";
import { store } from "@/store";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App store={store} />
  </React.StrictMode>,
);
