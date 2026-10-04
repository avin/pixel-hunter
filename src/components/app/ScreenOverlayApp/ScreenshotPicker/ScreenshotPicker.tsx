import { useEffect, useState } from "react";
import {
  usePixelCaptureCancelMutation,
  usePixelCapturePickMutation,
} from "@/store/api";
import { type Size, toPhysicalPoint } from "../physicalPoint";
import { Screenshot } from "../Screenshot/Screenshot";

interface Props {
  profileId: string;
  pixelId: string;
}

export function ScreenshotPicker({ profileId, pixelId }: Props) {
  const [pick, { isUninitialized }] = usePixelCapturePickMutation();
  const [cancel] = usePixelCaptureCancelMutation();
  const [size, setSize] = useState<Size>();

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        void cancel();
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [cancel]);

  return (
    <div
      role="application"
      aria-label="Выберите точку на снимке. Esc — отмена"
      className="fixed inset-0 cursor-crosshair overflow-hidden bg-black"
      // Pointer events carry fractional coordinates, unlike `click` in Chromium.
      onPointerDown={(event) => {
        if (event.button !== 0 || !size || !isUninitialized) {
          return;
        }
        void pick(
          toPhysicalPoint(event.clientX, event.clientY, window.devicePixelRatio, size),
        );
      }}
    >
      <Screenshot profileId={profileId} pixelId={pixelId} onLoad={setSize} />
    </div>
  );
}
