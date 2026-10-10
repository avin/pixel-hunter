import { Crosshair, ScanSearch } from "lucide-react";
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
    <section aria-label="Визуальное" className="flex min-h-0 min-w-0 flex-1 flex-col border">
      <ConditionHeader profileId={profileId} tracker={tracker}>
        <div className="flex shrink-0 items-center gap-2">
          {addButton}
          <Button size="sm" disabled={isCapturing} onClick={() => void startIconCapture({ profileId, trackerId: tracker.id })}><ScanSearch />Добавить иконку</Button>
        </div>
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
