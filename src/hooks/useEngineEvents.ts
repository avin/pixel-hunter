import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";
import { setEngineStatus, setIconStatus } from "@/store/engineSlice";
import { useAppDispatch } from "@/store/hooks";
import type { EngineStatus, IconStatus } from "@/store/types";

export const ENGINE_STATUS_EVENT = "engine://status";

/**
 * Mirrors engine status events into the store. Listens for the whole app lifetime, so the first
 * event after a profile is activated is never lost.
 */
export function useEngineEvents() {
  const dispatch = useAppDispatch();

  useEffect(() => {
    const unlisten = Promise.all([
      listen<EngineStatus>(ENGINE_STATUS_EVENT, ({ payload }) => dispatch(setEngineStatus(payload))),
      listen<IconStatus>("engine://icon-status", ({ payload }) => dispatch(setIconStatus(payload))),
    ]);
    return () => {
      void unlisten.then((stops) => stops.forEach((stop) => stop()));
    };
  }, [dispatch]);
}
