import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import {
  useLazyPickerFrameQuery,
  usePickerFrameDrawnMutation,
  usePickerFrameFailedMutation,
  usePickerReadyMutation,
  usePixelCaptureCancelMutation,
  usePixelCapturePickMutation,
  useScreenshotsDirQuery,
} from "@/store/api";
import type { PickerFrame } from "@/store/types";
import { type Size, toPhysicalPoint } from "../physicalPoint";
import { Screenshot } from "../Screenshot/Screenshot";

const FRAME_EVENT = "screen-overlay://frame";
const CLEAR_EVENT = "screen-overlay://clear";

export function ScreenshotPicker() {
  const { data: dir } = useScreenshotsDirQuery();
  const [ready] = usePickerReadyMutation();
  const [getFrame] = useLazyPickerFrameQuery();
  const [frame, setFrame] = useState<PickerFrame>();
  const [loaded, setLoaded] = useState<{ pixelId: string; size: Size }>();
  const [pick] = usePixelCapturePickMutation();
  const [cancel] = usePixelCaptureCancelMutation();
  const pickedId = useRef<string | undefined>(undefined);
  const size = loaded && frame && loaded.pixelId === frame.pixelId ? loaded.size : undefined;

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        void cancel();
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [cancel]);

  useEffect(() => {
    if (!dir) return;
    let isActive = true;
    const unlisten = Promise.all([
      listen<PickerFrame>(FRAME_EVENT, ({ payload }) => {
        if (isActive) setFrame(payload);
      }),
      listen(CLEAR_EVENT, () => {
        if (isActive) setFrame(undefined);
      }),
    ]).then(([stopFrame, stopClear]) => () => {
      stopFrame();
      stopClear();
    });
    void unlisten.then(async (stop) => {
      if (!isActive) {
        stop();
        return;
      }
      await ready();
      const result = await getFrame();
      if (isActive && result.data) setFrame(result.data);
    });
    return () => {
      isActive = false;
      void unlisten.then((stop) => stop());
    };
  }, [dir, getFrame, ready]);

  return (
    <div
      role="application"
      aria-label="Выберите точку на снимке. Esc — отмена"
      className="fixed inset-0 cursor-crosshair overflow-hidden bg-black"
      // Pointer events carry fractional coordinates, unlike `click` in Chromium.
      onPointerDown={(event) => {
        if (event.button !== 0 || !size || !frame || pickedId.current === frame.pixelId) {
          return;
        }
        pickedId.current = frame.pixelId;
        void pick(toPhysicalPoint(event.clientX, event.clientY, window.devicePixelRatio, size));
      }}
    >
      {frame ? (
        <PickerShot
          key={frame.pixelId}
          frame={frame}
          onLoad={(next) => setLoaded({ pixelId: frame.pixelId, size: next })}
        />
      ) : null}
    </div>
  );
}

interface Props {
  frame: PickerFrame;
  onLoad: (size: Size) => void;
}

function PickerShot({ frame, onLoad }: Props) {
  const [drawn] = usePickerFrameDrawnMutation();
  const [failed] = usePickerFrameFailedMutation();

  return (
    <Screenshot
      profileId={frame.profileId}
      pixelId={frame.pixelId}
      onLoad={onLoad}
      onReady={() => {
        void drawn(frame);
      }}
      onError={() => {
        void failed(frame);
      }}
    />
  );
}
