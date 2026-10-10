import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import {
  useLazyPickerFrameQuery,
  usePickerFrameDrawnMutation,
  usePickerFrameFailedMutation,
  usePickerReadyMutation,
  usePixelCaptureCancelMutation,
  usePixelCapturePickMutation,
  useIconCaptureSelectMutation,
  useScreenshotsDirQuery,
} from "@/store/api";
import type { PhysicalRect, PickerFrame } from "@/store/types";
import { type Point, type Size, toPhysicalPoint, toPhysicalRect } from "../physicalPoint";
import { Screenshot } from "../Screenshot/Screenshot";
import { CursorHint } from "../CursorHint/CursorHint";
import { RegionMarker } from "../RegionMarker/RegionMarker";

const FRAME_EVENT = "screen-overlay://frame";
const CLEAR_EVENT = "screen-overlay://clear";

export function ScreenshotPicker() {
  const { data: dir } = useScreenshotsDirQuery();
  const [ready] = usePickerReadyMutation();
  const [getFrame] = useLazyPickerFrameQuery();
  const [frame, setFrame] = useState<PickerFrame>();
  const [cancel] = usePixelCaptureCancelMutation();

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

  return <PickerShot key={frame?.pixelId ?? "empty"} frame={frame} />;
}

interface Props {
  frame?: PickerFrame;
}

function PickerShot({ frame }: Props) {
  const [drawn] = usePickerFrameDrawnMutation();
  const [failed] = usePickerFrameFailedMutation();
  const [pick] = usePixelCapturePickMutation();
  const [select] = useIconCaptureSelectMutation();
  const [size, setSize] = useState<Size>();
  const [cursor, setCursor] = useState<Point>({ x: 0, y: 0 });
  const [drag, setDrag] = useState<{ start: Point; end: Point }>();
  const dragStart = useRef<Point | undefined>(undefined);
  const [searchRect, setSearchRect] = useState<PhysicalRect>();
  const [error, setError] = useState<string>();
  const [isSaving, setIsSaving] = useState(false);
  const picked = useRef(false);
  const isIcon = frame?.mode === "icon";
  const stage = searchRect ? "template" : "search";
  const hint = !isIcon ? "выбрать пиксель" : searchRect ? "выбрать область иконки" : "выбрать область для поиска иконки";
  const selection = size && drag ? toPhysicalRect(drag.start, drag.end, window.devicePixelRatio, size) : undefined;

  return (
    <div role="application" aria-label={isIcon ? "Выберите область поиска и образец иконки. Esc — отмена" : "Выберите точку на снимке. Esc — отмена"} className="fixed inset-0 cursor-crosshair overflow-hidden bg-black"
      onPointerMove={(event) => {
        const next = { x: event.clientX, y: event.clientY };
        setCursor(next);
        if (dragStart.current) setDrag({ start: dragStart.current, end: next });
      }}
      onPointerDown={(event) => {
        if (event.button !== 0 || !frame || !size || isSaving || picked.current) return;
        const point = { x: event.clientX, y: event.clientY };
        setCursor(point);
        if (!isIcon) {
          picked.current = true;
          void pick(toPhysicalPoint(point.x, point.y, window.devicePixelRatio, size));
          return;
        }
        setError(undefined);
        dragStart.current = point;
        setDrag({ start: point, end: point });
        event.currentTarget.setPointerCapture?.(event.pointerId);
      }}
      onPointerCancel={() => { dragStart.current = undefined; setDrag(undefined); }}
      onPointerUp={async (event) => {
        const start = dragStart.current;
        dragStart.current = undefined;
        setDrag(undefined);
        if (!start || !size || !frame || isSaving) return;
        const rect = toPhysicalRect(start, { x: event.clientX, y: event.clientY }, window.devicePixelRatio, size);
        if (rect.width === 0 || rect.height === 0) {
          setError("Выделите область перетаскиванием");
          return;
        }
        setIsSaving(true);
        const result = await select({ pixelId: frame.pixelId, rect, stage });
        setIsSaving(false);
        if (result.error) {
          setError("message" in result.error ? String(result.error.message) : "Не удалось сохранить выделение");
        } else if (result.data.stage === "template") {
          setSearchRect(result.data.searchRect ?? rect);
        } else {
          picked.current = true;
        }
      }}>
      {frame && <Screenshot profileId={frame.profileId} pixelId={frame.pixelId} onLoad={setSize} onReady={() => void drawn(frame)} onError={() => void failed(frame)}>
        {(imageSize) => <>
          {searchRect && <RegionMarker rect={searchRect} size={imageSize} kind="search" />}
          {selection && <RegionMarker rect={selection} size={imageSize} kind={stage === "search" ? "search" : "template"} />}
        </>}
      </Screenshot>}
      {frame && <CursorHint cursor={cursor} text={isSaving ? "сохраняем выделение" : hint} />}
      {error && <p role="alert" className="pointer-events-none absolute bottom-4 left-4 max-w-full bg-gray-200 px-3 py-2 text-black">{error}</p>}
    </div>
  );
}
