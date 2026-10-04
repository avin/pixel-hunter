import { Crosshair } from "lucide-react";
import { useId } from "react";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableBody,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  usePixelCaptureCancelMutation,
  usePixelCaptureStartMutation,
  useScreenshotsDirQuery,
} from "@/store/api";
import { useAppDispatch, useAppSelector } from "@/store/hooks";
import {
  dismissCaptureStatus,
  isCapturingSelector,
  trackerCaptureStatusSelector,
} from "@/store/pixelCaptureSlice";
import type { Tracker } from "@/store/types";
import { CaptureStatusPanel } from "./CaptureStatusPanel/CaptureStatusPanel";
import { PixelRow } from "./PixelRow/PixelRow";

interface Props {
  profileId: string;
  processName: string;
  tracker: Tracker;
}

export function PixelsSection({ profileId, processName, tracker }: Props) {
  const dispatch = useAppDispatch();
  const isCapturing = useAppSelector(isCapturingSelector);
  const status = useAppSelector((state) =>
    trackerCaptureStatusSelector(state, tracker.id),
  );
  const [startCapture] = usePixelCaptureStartMutation();
  const [cancelCapture] = usePixelCaptureCancelMutation();
  const { data: screenshotsDir } = useScreenshotsDirQuery();
  const headingId = useId();

  const addButton = (
    <Button
      size="sm"
      disabled={isCapturing}
      onClick={() => void startCapture({ profileId, trackerId: tracker.id })}
    >
      <Crosshair />
      Добавить пиксель
    </Button>
  );

  return (
    <section aria-labelledby={headingId} className="flex min-h-0 min-w-0 flex-1 flex-col border">
      <div className="flex shrink-0 items-center justify-between gap-2 border-b px-3 py-2">
        <h2 id={headingId} className="font-heading text-sm font-medium">
          Пиксели
        </h2>
        {addButton}
      </div>

      {status && (
        <CaptureStatusPanel
          status={status}
          processName={processName}
          onCancel={() => void cancelCapture()}
          onDismiss={() => dispatch(dismissCaptureStatus())}
        />
      )}

      {tracker.pixels.length === 0 ? (
        <div className="overflow-auto px-3 py-3">
          <h3 className="text-sm font-medium">Пикселей пока нет</h3>
          <p className="mt-1 text-xs text-muted-foreground">
            После нажатия переключитесь в игру: через 3 секунды приложение сделает
            снимок экрана, и на нём можно будет выбрать точку.
          </p>
        </div>
      ) : (
        <div className="min-h-0 flex-1 overflow-hidden [&>div]:h-full [&>div]:overflow-auto">
          <Table aria-label="Пиксели" className="min-w-[560px] text-xs [&_td]:py-1 [&_th]:h-8">
            <TableHeader className="sticky top-0 z-10 bg-background">
              <TableRow>
                <TableHead className="w-20">Снимок</TableHead>
                <TableHead className="w-28">Координаты</TableHead>
                <TableHead className="w-28">Цвет</TableHead>
                <TableHead className="w-32">Условие</TableHead>
                <TableHead className="w-24">Допуск, %</TableHead>
                <TableHead className="w-10">
                  <span className="sr-only">Действия</span>
                </TableHead>
              </TableRow>
            </TableHeader>
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
            </TableBody>
          </Table>
        </div>
      )}
    </section>
  );
}
