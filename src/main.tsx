import React from "react";
import ReactDOM from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import App from "./App";
import { Palette } from "./Palette";
import { Reminder } from "./Reminder";
import { isMac } from "./lib/shortcut";
import "./styles.css";

// Les données sont locales et rafraîchies par les événements Rust : pas besoin de refetch auto.
const queryClient = new QueryClient({
  defaultOptions: {
    queries: { staleTime: Infinity, refetchOnWindowFocus: false, retry: false },
  },
});

// Ouvert dans un simple navigateur pendant le développement : faux backend en mémoire.
if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) {
  const { installMockBackend } = await import("./dev/mockBackend");
  installMockBackend();
}

// Même bundle pour toutes les fenêtres ; l'ancre de l'URL choisit la vue.
const view = window.location.hash.slice(1);
if (isMac && view === "") document.documentElement.classList.add("mac");
if (view === "palette") document.body.classList.add("palette-window");
if (view === "reminder") document.body.classList.add("reminder-window");

function Root() {
  if (view === "palette") return <Palette />;
  if (view === "reminder") return <Reminder />;
  return <App />;
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <Root />
    </QueryClientProvider>
  </React.StrictMode>,
);
