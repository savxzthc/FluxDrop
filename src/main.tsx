import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles/main.css";

const storedTheme = window.localStorage.getItem("fluxdrop-theme");
const initialTheme =
  storedTheme === "dark" || storedTheme === "light"
    ? storedTheme
    : window.matchMedia("(prefers-color-scheme: dark)").matches
      ? "dark"
      : "light";
document.documentElement.dataset.theme = initialTheme;
document.documentElement.style.colorScheme = initialTheme;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
