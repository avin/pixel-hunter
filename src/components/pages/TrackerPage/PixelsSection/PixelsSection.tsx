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
      disabled={isCapturing}
      onClick={() => void startCapture({ profileId, trackerId: tracker.id })}
    >
      <Crosshair />
      Добавить пиксель
    </Button>
  );

  return (
    <section aria-labelledby={headingId} className="flex flex-col gap-4 border-t p-6">
      <div className="flex items-center justify-between gap-4">
        <h2 id={headingId} className="font-heading text-base font-medium">
          Пиксели
        </h2>
        {tracker.pixels.length > 0 && addButton}
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
        <div className="flex flex-col items-center gap-3 rounded-xl border border-dashed px-6 py-12 text-center">
          <h3 className="font-heading text-base font-medium">Пикселей пока нет</h3>
          <p className="max-w-sm text-sm text-muted-foreground">
            После нажатия переключитесь в игру: через 3 секунды приложение сделает
            снимок экрана, и на нём можно будет выбрать точку.
          </p>
          {addButton}
        </div>
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Снимок</TableHead>
              <TableHead>Координаты</TableHead>
              <TableHead>Цвет</TableHead>
              <TableHead>Условие</TableHead>
              <TableHead>Допуск, %</TableHead>
              <TableHead className="w-0">
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
      )}
    </section>
  );
}
