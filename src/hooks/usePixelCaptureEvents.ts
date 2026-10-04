import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";
import { tauriApi } from "@/store/api";
import { useAppDispatch } from "@/store/hooks";
import { setCaptureStatus } from "@/store/pixelCaptureSlice";
import type { CaptureStatus } from "@/store/types";

export const CAPTURE_STATUS_EVENT = "pixel-capture://status";

/** Mirrors capture status events into the store; a finished capture refreshes profiles. */
export function usePixelCaptureEvents() {
  const dispatch = useAppDispatch();

  useEffect(() => {
    const unlisten = listen<CaptureStatus>(CAPTURE_STATUS_EVENT, ({ payload }) => {
      dispatch(setCaptureStatus(payload));
      if (payload.phase === "done") {
        dispatch(tauriApi.util.invalidateTags(["Profile"]));
      }
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [dispatch]);
}
