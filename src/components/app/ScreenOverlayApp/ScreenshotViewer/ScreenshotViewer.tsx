import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import {
  useLazyScreenshotViewRequestQuery,
  useScreenshotViewCloseMutation,
  useScreenshotViewFailedMutation,
  useScreenshotViewReadyMutation,
} from "@/store/api";
import type { ScreenshotViewRequest } from "@/store/types";
import type { Point, Size } from "../physicalPoint";
import { Screenshot } from "../Screenshot/Screenshot";

const RING_RADIUS = 16;
const RING_WIDTH = 3;
const CROSS_REACH = 24;

export function ScreenshotViewer() {
  const [request, setRequest] = useState<ScreenshotViewRequest>();
  const [getRequest] = useLazyScreenshotViewRequestQuery();

  useEffect(() => {
    let isActive = true;
    function receive(next: ScreenshotViewRequest | null) {
      if (!isActive || !next) return;
      setRequest((current) => !current || next.sequence > current.sequence ? next : current);
    }
    // Subscribe first: the initial request may arrive while the WebView is booting.
    const unlisten = listen<ScreenshotViewRequest>("screenshot-viewer://open", ({ payload }) => receive(payload));
    void unlisten.then(() => {
      if (isActive) void getRequest().then((result) => {
        if (result.data) receive(result.data);
      });
    });
    return () => {
      isActive = false;
      void unlisten.then((stop) => stop());
    };
  }, [getRequest]);

  return request ? <ScreenshotPreview key={request.sequence} request={request} /> : null;
}

interface Props { request: ScreenshotViewRequest }

function ScreenshotPreview({ request }: Props) {
  const [show] = useScreenshotViewReadyMutation();
  const [hide] = useScreenshotViewCloseMutation();
  const [fail] = useScreenshotViewFailedMutation();
  const isClosing = useRef(false);

  useEffect(() => {
    const close = (event: Event) => {
      event.preventDefault();
      event.stopPropagation();
      if (isClosing.current) return;
      isClosing.current = true;
      void hide({ sequence: request.sequence });
    };
    window.addEventListener("pointerdown", close, true);
    window.addEventListener("keydown", close, true);
    return () => {
      window.removeEventListener("pointerdown", close, true);
      window.removeEventListener("keydown", close, true);
    };
  }, [hide, request.sequence]);

  return (
    <div
      role="dialog"
      aria-label="Просмотр снимка. Любой клик или клавиша — закрыть"
      aria-modal="true"
      className="fixed inset-0 flex items-center justify-center overflow-hidden bg-black"
    >
      <Screenshot
        profileId={request.profileId}
        pixelId={request.pixelId}
        isFitted
        onReady={() => {
          if (!isClosing.current) void show({ sequence: request.sequence });
        }}
        onError={() => {
          isClosing.current = true;
          void fail({ sequence: request.sequence });
        }}
      >
        {(size) => <PixelMarker pixel={{ x: request.x, y: request.y }} size={size} />}
      </Screenshot>
    </div>
  );
}

interface PixelMarkerProps {
  pixel: Point;
  size: Size;
}

/** Image-space coordinates keep the marker aligned at any preview scale. */
function PixelMarker({ pixel, size }: PixelMarkerProps) {
  const { width, height } = size;
  const centerX = pixel.x + 0.5;
  const centerY = pixel.y + 0.5;

  return (
    <svg
      role="img"
      aria-label={`Пиксель (${pixel.x}, ${pixel.y})`}
      width="100%"
      height="100%"
      viewBox={`0 0 ${width} ${height}`}
      className="pointer-events-none absolute top-0 left-0 text-red-500 animate-pixel-marker-color"
    >
      <g
        className="animate-pixel-marker-motion"
        style={{ transformOrigin: `${centerX}px ${centerY}px` }}
        stroke="currentColor"
        strokeWidth={1}
      >
        <circle
          cx={centerX}
          cy={centerY}
          r={RING_RADIUS}
          fill="none"
          strokeWidth={RING_WIDTH}
        />
        <line x1={centerX - CROSS_REACH} y1={centerY} x2={centerX + CROSS_REACH} y2={centerY} />
        <line x1={centerX} y1={centerY - CROSS_REACH} x2={centerX} y2={centerY + CROSS_REACH} />
      </g>
    </svg>
  );
}
