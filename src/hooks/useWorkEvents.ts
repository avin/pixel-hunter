import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";
import { useAppDispatch } from "@/store/hooks";
import type { WorkPause } from "@/store/types";
import { setWorkPaused } from "@/store/workSlice";

export const WORK_PAUSE_EVENT = "work://pause";

/**
 * Mirrors pause events into the store for the whole app lifetime, including a hotkey press
 * while another window is in front.
 */
export function useWorkEvents() {
  const dispatch = useAppDispatch();

  useEffect(() => {
    const unlisten = listen<WorkPause>(WORK_PAUSE_EVENT, ({ payload }) => {
      dispatch(setWorkPaused(payload.paused));
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [dispatch]);
}
