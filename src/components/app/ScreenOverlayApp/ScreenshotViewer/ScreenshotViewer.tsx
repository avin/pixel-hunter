import { listen } from "@tauri-apps/api/event";
import { cn } from "cn";
import { type CSSProperties, useEffect, useRef, useState } from "react";
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

/** The second callback runs after the browser has painted the preceding frame. */
function waitForPaint() {
  return new Promise<void>((resolve) => {
    window.requestAnimationFrame(() => window.requestAnimationFrame(() => resolve()));
  });
}

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
  const [isShown, setIsShown] = useState(false);
  const [show] = useScreenshotViewReadyMutation();
  const [hide] = useScreenshotViewCloseMutation();
  const [fail] = useScreenshotViewFailedMutation();
  const isClosing = useRef(false);

  useEffect(() => {
    const close = async (event: Event) => {
      event.preventDefault();
      event.stopPropagation();
      if (isClosing.current) return;
      isClosing.current = true;
      // Clear the retained WebView surface before hiding this reusable window.
      setIsShown(false);
      await waitForPaint();
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
      className="fixed inset-0 flex items-center justify-center overflow-hidden"
    >
      <div
        className={cn("origin-center shadow-screenshot", isShown && "animate-screenshot-open")}
        style={{
          opacity: 0,
          ...(request.origin && {
            "--screenshot-origin-x": `calc(${request.origin.x}px - 50vw)`,
            "--screenshot-origin-y": `calc(${request.origin.y}px - 50vh)`,
          }),
        } as CSSProperties}
      >
        <Screenshot
          profileId={request.profileId}
          pixelId={request.pixelId}
          isFitted
          onReady={async () => {
            if (isClosing.current) return;
            const result = await show({ sequence: request.sequence });
            if ("error" in result || isClosing.current) return;
            // Hidden WebViews can suspend frames; wait only after the window is visible.
            await waitForPaint();
            if (!isClosing.current) setIsShown(true);
          }}
          onError={() => {
            isClosing.current = true;
            void fail({ sequence: request.sequence });
          }}
        >
          {(size) => <PixelMarker pixel={{ x: request.x, y: request.y }} size={size} />}
        </Screenshot>
      </div>
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
