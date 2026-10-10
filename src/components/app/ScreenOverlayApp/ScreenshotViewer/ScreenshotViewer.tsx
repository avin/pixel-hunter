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
import { RegionMarker } from "../RegionMarker/RegionMarker";

const MARKER_SIZE = 104;
const MARKER_CENTER = MARKER_SIZE / 2;
const RING_RADIUS = 32;
const CROSS_REACH = 48;
const CROSS_GAP = 8;
const MARKER_LAYERS = [
  { color: "var(--color-black)", width: 9 },
  { color: "var(--color-white)", width: 6 },
  { color: undefined, width: 4 },
];

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
          {(size) => request.marker?.kind === "icon" ? <>
            <RegionMarker rect={request.marker.searchRect} size={size} kind="search" isPreview />
            <RegionMarker rect={request.marker.templateRect} size={size} kind="template" isPreview />
          </> : <PixelMarker pixel={request.marker?.kind === "pixel" ? request.marker : { x: request.x, y: request.y }} size={size} />}
        </Screenshot>
      </div>
    </div>
  );
}

interface PixelMarkerProps {
  pixel: Point;
  size: Size;
}

/** Image-relative positioning keeps the center aligned; size stays readable at any preview scale. */
function PixelMarker({ pixel, size }: PixelMarkerProps) {
  const { width, height } = size;
  const center = MARKER_CENTER;
  const cross = [
    `M ${center - CROSS_REACH} ${center} H ${center - CROSS_GAP}`,
    `M ${center + CROSS_GAP} ${center} H ${center + CROSS_REACH}`,
    `M ${center} ${center - CROSS_REACH} V ${center - CROSS_GAP}`,
    `M ${center} ${center + CROSS_GAP} V ${center + CROSS_REACH}`,
  ].join(" ");

  return (
    <svg
      role="img"
      aria-label={`Пиксель (${pixel.x}, ${pixel.y})`}
      width={MARKER_SIZE}
      height={MARKER_SIZE}
      viewBox={`0 0 ${MARKER_SIZE} ${MARKER_SIZE}`}
      className="pointer-events-none absolute -translate-x-1/2 -translate-y-1/2 overflow-visible"
      style={{ left: `${(pixel.x + 0.5) / width * 100}%`, top: `${(pixel.y + 0.5) / height * 100}%` }}
    >
      <g
        className="animate-pixel-marker-motion"
        style={{ transformOrigin: `${center}px ${center}px` }}
        fill="none"
        strokeLinecap="round"
      >
        {MARKER_LAYERS.map((layer) => (
          <g key={layer.width} strokeWidth={layer.width}>
            <circle
              cx={center}
              cy={center}
              r={RING_RADIUS}
              stroke={layer.color ?? "var(--color-pixel-marker-ring)"}
              className={layer.color ? undefined : "animate-preview-marker-color"}
            />
            <path
              d={cross}
              stroke={layer.color ?? "var(--color-pixel-marker-cross)"}
              className={layer.color ? undefined : "animate-preview-marker-color"}
              style={layer.color ? undefined : { animationDelay: "-1.5s" }}
            />
          </g>
        ))}
      </g>
    </svg>
  );
}
