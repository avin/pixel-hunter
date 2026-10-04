import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect } from "react";
import type { Point, Size } from "../physicalPoint";
import { Screenshot } from "../Screenshot/Screenshot";

interface Props {
  profileId: string;
  pixelId: string;
  pixel: Point;
}

const RING_RADIUS = 16;
const RING_WIDTH = 3;
const CROSS_REACH = 24;

export function ScreenshotViewer({ profileId, pixelId, pixel }: Props) {
  useEffect(() => {
    const close = () => void getCurrentWindow().close();
    window.addEventListener("pointerdown", close);
    window.addEventListener("keydown", close);
    return () => {
      window.removeEventListener("pointerdown", close);
      window.removeEventListener("keydown", close);
    };
  }, []);

  return (
    <div className="fixed inset-0 overflow-hidden bg-black">
      <Screenshot profileId={profileId} pixelId={pixelId}>
        {(size) => <PixelMarker pixel={pixel} size={size} />}
      </Screenshot>
    </div>
  );
}

interface PixelMarkerProps {
  pixel: Point;
  size: Size;
}

/** Ring with a cross whose one-physical-pixel lines intersect on the picked pixel. */
function PixelMarker({ pixel, size }: PixelMarkerProps) {
  const dpr = window.devicePixelRatio;
  const width = size.width / dpr;
  const height = size.height / dpr;
  const centerX = (pixel.x + 0.5) / dpr;
  const centerY = (pixel.y + 0.5) / dpr;

  return (
    <svg
      role="img"
      aria-label={`Пиксель (${pixel.x}, ${pixel.y})`}
      width={width}
      height={height}
      viewBox={`0 0 ${width} ${height}`}
      className="pointer-events-none absolute top-0 left-0 text-red-500"
    >
      <circle
        cx={centerX}
        cy={centerY}
        r={RING_RADIUS}
        fill="none"
        stroke="currentColor"
        strokeWidth={RING_WIDTH}
      />
      <g stroke="currentColor" strokeWidth={1 / dpr} shapeRendering="crispEdges">
        <line x1={centerX - CROSS_REACH} y1={centerY} x2={centerX + CROSS_REACH} y2={centerY} />
        <line x1={centerX} y1={centerY - CROSS_REACH} x2={centerX} y2={centerY + CROSS_REACH} />
      </g>
    </svg>
  );
}
