import { Crosshair, ScanSearch } from "lucide-react";
import { useEffect } from "react";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableBody,
} from "@/components/ui/table";
import { ConditionHeader } from "@/components/pages/TrackerPage/ConditionSection/ConditionSection";
import {
  usePixelCaptureCancelMutation,
  usePixelCaptureStartMutation,
  useIconCaptureStartMutation,
  useScreenshotsDirQuery,
  tauriApi,
} from "@/store/api";
import { useAppDispatch, useAppSelector, useAppStore } from "@/store/hooks";
import {
  dismissCaptureStatus,
  isCapturingSelector,
  trackerCaptureStatusSelector,
  captureIsWaiting,
} from "@/store/pixelCaptureSlice";
import type { Tracker } from "@/store/types";
import { CaptureStatusPanel } from "./CaptureStatusPanel/CaptureStatusPanel";
import { PixelRow } from "./PixelRow/PixelRow";
import { IconRow } from "./IconRow/IconRow";

interface Props {
  profileId: string;
  tracker: Tracker;
}

export function PixelsSection({ profileId, tracker }: Props) {
  const dispatch = useAppDispatch();
  const store = useAppStore();
  const isCapturing = useAppSelector(isCapturingSelector);
  const status = useAppSelector((state) =>
    trackerCaptureStatusSelector(state, profileId, tracker.id),
  );
  const [startCapture] = usePixelCaptureStartMutation();
  const [startIconCapture] = useIconCaptureStartMutation();
  const [cancelCapture] = usePixelCaptureCancelMutation();
  const { data: screenshotsDir } = useScreenshotsDirQuery();

  useEffect(() => () => {
    const { active, status: currentStatus } = store.getState().pixelCapture;
    if (active?.profileId === profileId && active.trackerId === tracker.id && captureIsWaiting(currentStatus)) {
      void dispatch(tauriApi.endpoints.pixelCaptureCancel.initiate({ sessionId: active.sessionId }));
    }
  }, [dispatch, profileId, store, tracker.id]);

  const isPixelActive = isCapturing && status?.mode === "pixel";
  const isIconActive = isCapturing && status?.mode === "icon";
  function cancelWaiting() {
    if (status && captureIsWaiting(status)) void cancelCapture({ sessionId: status.sessionId });
  }

  const addButton = (
    <Button
      size="sm"
      variant={isPixelActive ? "secondary" : "default"}
      aria-pressed={isPixelActive}
      disabled={isCapturing && (!isPixelActive || !captureIsWaiting(status))}
      onClick={() => isPixelActive ? cancelWaiting() : void startCapture({ profileId, trackerId: tracker.id, sessionId: crypto.randomUUID() })}
    >
      <Crosshair />
      Добавить пиксель
    </Button>
  );

  return (
    <section aria-label="Визуальное" className="flex min-h-0 min-w-0 flex-1 flex-col border">
      <ConditionHeader profileId={profileId} tracker={tracker}>
        <div className="flex min-w-0 flex-wrap items-center gap-2">
          {(captureIsWaiting(status) || status?.phase === "capturing") && (
            <p role="status" aria-label="Состояние захвата" className="whitespace-nowrap text-xs text-muted-foreground">
              Нажмите <span className="animate-capture-key-blink font-semibold text-capture-key-alert motion-reduce:animate-none">PrintScreen</span> в игре
            </p>
          )}
          {addButton}
          <Button size="sm" variant={isIconActive ? "secondary" : "default"} aria-pressed={isIconActive} disabled={isCapturing && (!isIconActive || !captureIsWaiting(status))} onClick={() => isIconActive ? cancelWaiting() : void startIconCapture({ profileId, trackerId: tracker.id, sessionId: crypto.randomUUID() })}><ScanSearch />Добавить иконку</Button>
        </div>
      </ConditionHeader>

      {status && !captureIsWaiting(status) && status.phase !== "capturing" && status.phase !== "saving" && (
        <CaptureStatusPanel
          status={status}
          onDismiss={() => dispatch(dismissCaptureStatus())}
        />
      )}

      {tracker.pixels.length === 0 && tracker.icons.length === 0 ? (
        <div className="overflow-auto px-3 py-3">
          <h3 className="text-sm font-medium">Пикселей и иконок пока нет</h3>
          <p className="mt-1 text-xs text-muted-foreground">
            Добавьте пиксель или иконку, подготовьте экран и нажмите PrintScreen, чтобы сделать снимок.
            Для иконки сначала выделите область поиска, затем образец внутри неё.
          </p>
        </div>
      ) : (
        <div className="min-h-0 flex-1 overflow-auto">
          <Table aria-label="Визуальные условия" className="min-w-[640px] text-xs [&_td]:py-2">
            <TableBody>
              {tracker.pixels.map((pixel) => (
                <PixelRow
                  key={pixel.id}
                  profileId={profileId}
                  trackerId={tracker.id}
                  pixel={pixel}
                  screenshotsDir={screenshotsDir}
                />
              ))}
              {tracker.icons.map((icon) => (
                <IconRow
                  key={icon.id}
                  profileId={profileId}
                  trackerId={tracker.id}
                  icon={icon}
                  screenshotsDir={screenshotsDir}
                />
              ))}
            </TableBody>
          </Table>
        </div>
      )}
    </section>
  );
}
