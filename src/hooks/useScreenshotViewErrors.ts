import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";
import { toast } from "sonner";

export function useScreenshotViewErrors() {
  useEffect(() => {
    const unlisten = listen<string>("screenshot-viewer://error", ({ payload }) => toast.error(payload));
    return () => { void unlisten.then((stop) => stop()); };
  }, []);
}
