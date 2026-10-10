import { Crosshair, ScanSearch } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableBody,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { ConditionHeader } from "@/components/pages/TrackerPage/ConditionSection/ConditionSection";
import {
  usePixelCaptureCancelMutation,
  usePixelCaptureStartMutation,
  useIconCaptureStartMutation,
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
import { IconRow } from "./IconRow/IconRow";

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
  const [startIconCapture] = useIconCaptureStartMutation();
  const [cancelCapture] = usePixelCaptureCancelMutation();
  const { data: screenshotsDir } = useScreenshotsDirQuery();

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
    <section aria-label="Пиксели" className="flex min-h-0 min-w-0 flex-1 flex-col border">
      <ConditionHeader profileId={profileId} tracker={tracker}>
        {addButton}
        <Button size="sm" disabled={isCapturing} onClick={() => void startIconCapture({ profileId, trackerId: tracker.id })}><ScanSearch />Добавить иконку</Button>
      </ConditionHeader>

      {status && (
        <CaptureStatusPanel
          status={status}
          processName={processName}
          onCancel={() => void cancelCapture()}
          onDismiss={() => dispatch(dismissCaptureStatus())}
        />
      )}

      {tracker.pixels.length === 0 && tracker.icons.length === 0 ? (
        <div className="overflow-auto px-3 py-3">
          <h3 className="text-sm font-medium">Пикселей и иконок пока нет</h3>
          <p className="mt-1 text-xs text-muted-foreground">
            Добавьте пиксель или иконку. Переключитесь в игру, чтобы сделать снимок.
            Для иконки сначала выделите область поиска, затем образец внутри неё.
          </p>
        </div>
      ) : (
        <div className="min-h-0 flex-1 overflow-auto">
          {tracker.pixels.length > 0 && <Table aria-label="Пиксели" className="min-w-[560px] text-xs [&_td]:py-1 [&_th]:h-8">
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
          </Table>}
          {tracker.icons.length > 0 && <Table aria-label="Иконки" className="min-w-[560px] text-xs [&_td]:py-1 [&_th]:h-8">
            <TableHeader className="sticky top-0 z-10 bg-background"><TableRow>
              <TableHead className="w-20">Снимок</TableHead><TableHead>Образец</TableHead><TableHead>Условие</TableHead><TableHead>Сходство, %</TableHead><TableHead><span className="sr-only">Действия</span></TableHead>
            </TableRow></TableHeader>
            <TableBody>{tracker.icons.map((icon) => <IconRow key={icon.id} profileId={profileId} trackerId={tracker.id} icon={icon} screenshotsDir={screenshotsDir} />)}</TableBody>
          </Table>}
        </div>
      )}
      <p className="shrink-0 border-t px-3 py-2 text-xs text-muted-foreground">Все пиксели и иконки проверяются вместе (AND). Иконка ищется в выбранной области в сохранённом размере, без масштабирования и поворота. Порог сходства 1–100% — техническая оценка, не вероятность распознавания.</p>
    </section>
  );
}
