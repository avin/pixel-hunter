import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";
import { setEngineStatus } from "@/store/engineSlice";
import { useAppDispatch } from "@/store/hooks";
import type { EngineStatus } from "@/store/types";

export const ENGINE_STATUS_EVENT = "engine://status";

/**
 * Mirrors engine status events into the store. Listens for the whole app lifetime, so the first
 * event after a profile is activated is never lost.
 */
export function useEngineEvents() {
  const dispatch = useAppDispatch();

  useEffect(() => {
    const unlisten = listen<EngineStatus>(ENGINE_STATUS_EVENT, ({ payload }) => {
      dispatch(setEngineStatus(payload));
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [dispatch]);
}
