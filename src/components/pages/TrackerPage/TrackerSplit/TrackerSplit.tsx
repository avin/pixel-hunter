import { useEffect, useRef, useState, type ReactNode } from "react";
import { cn } from "cn";

interface Props {
  pixels: ReactNode;
  action: ReactNode;
}

const STORAGE_KEY = "pixel-hunter.tracker-editor.pixels-height-percent.v1";
const MIN_PERCENT = 20;
const MAX_PERCENT = 80;
// Keep the section heading, capture/type controls and add command visible.
const MIN_SECTION_HEIGHT = 144;
let sessionPercent = 50;

function readPercent() {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored === null || stored.trim() === "") return 50;
    const percent = Number(stored);
    return Number.isFinite(percent) && percent >= MIN_PERCENT && percent <= MAX_PERCENT
      ? percent
      : 50;
  } catch {
    return sessionPercent;
  }
}

function savePercent(percent: number) {
  sessionPercent = percent;
  try {
    localStorage.setItem(STORAGE_KEY, String(percent));
  } catch {
    // The current session remains usable when storage is unavailable.
  }
}

function limits(height: number) {
  const minimum = height > 0 ? Math.min(50, (MIN_SECTION_HEIGHT / height) * 100) : MIN_PERCENT;
  return { min: Math.max(MIN_PERCENT, minimum), max: Math.min(MAX_PERCENT, 100 - minimum) };
}

function clamp(percent: number, height: number) {
  const { min, max } = limits(height);
  return Math.min(max, Math.max(min, percent));
}

export function TrackerSplit({ pixels, action }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const handleRef = useRef<HTMLDivElement>(null);
  const dragRef = useRef<{ pointerId: number; startY: number; percent: number; height: number } | null>(null);
  const percentRef = useRef(50);
  const [percent, setPercent] = useState(readPercent);
  const [availableHeight, setAvailableHeight] = useState(0);
  const [isDragging, setIsDragging] = useState(false);
  percentRef.current = percent;

  function measureHeight() {
    const container = containerRef.current;
    const handle = handleRef.current;
    if (!container || !handle) return 0;
    return Math.max(0, container.getBoundingClientRect().height - handle.getBoundingClientRect().height);
  }

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const measure = () => setAvailableHeight(measureHeight());
    measure();
    if (typeof ResizeObserver === "undefined") {
      window.addEventListener("resize", measure);
      return () => window.removeEventListener("resize", measure);
    }
    const observer = new ResizeObserver(measure);
    observer.observe(container);
    return () => observer.disconnect();
  }, []);

  const displayedPercent = clamp(percent, availableHeight);
  const { min, max } = limits(availableHeight);

  function finishDrag() {
    if (!dragRef.current) return;
    dragRef.current = null;
    setIsDragging(false);
    savePercent(percentRef.current);
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-auto p-2">
      <div
        ref={containerRef}
        className={cn("grid min-h-0 flex-1", { "select-none": isDragging })}
        style={{
          gridTemplateRows: `minmax(${MIN_SECTION_HEIGHT}px, ${displayedPercent}fr) auto minmax(${MIN_SECTION_HEIGHT}px, ${100 - displayedPercent}fr)`,
        }}
      >
        {pixels}
        <div
          ref={handleRef}
          role="separator"
          tabIndex={0}
          aria-label="Высота области пикселей"
          aria-orientation="horizontal"
          aria-valuemin={min}
          aria-valuemax={max}
          aria-valuenow={displayedPercent}
          aria-valuetext={`${Math.round(displayedPercent)}% для пикселей`}
          className="h-2 touch-none cursor-row-resize outline-none focus-visible:ring-2 focus-visible:ring-ring"
          onKeyDown={(event) => {
            if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;
            event.preventDefault();
            const next = clamp(displayedPercent + (event.key === "ArrowUp" ? -1 : 1), measureHeight());
            setPercent(next);
            savePercent(next);
          }}
          onPointerDown={(event) => {
            if (event.button !== 0 || dragRef.current) return;
            const height = measureHeight();
            if (height <= 0) return;
            event.preventDefault();
            event.currentTarget.focus();
            event.currentTarget.setPointerCapture(event.pointerId);
            dragRef.current = { pointerId: event.pointerId, startY: event.clientY, percent: clamp(percent, height), height };
            setIsDragging(true);
          }}
          onPointerMove={(event) => {
            const drag = dragRef.current;
            if (!drag || drag.pointerId !== event.pointerId) return;
            const next = clamp(drag.percent + ((event.clientY - drag.startY) / drag.height) * 100, measureHeight());
            percentRef.current = next;
            setPercent(next);
          }}
          onPointerUp={(event) => {
            if (dragRef.current?.pointerId !== event.pointerId) return;
            finishDrag();
            event.currentTarget.releasePointerCapture(event.pointerId);
          }}
          onPointerCancel={(event) => {
            if (dragRef.current?.pointerId !== event.pointerId) return;
            finishDrag();
            event.currentTarget.releasePointerCapture(event.pointerId);
          }}
          onLostPointerCapture={finishDrag}
        />
        {action}
      </div>
    </div>
  );
}
