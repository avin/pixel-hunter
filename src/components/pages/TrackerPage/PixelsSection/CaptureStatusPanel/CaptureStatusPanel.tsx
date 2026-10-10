import { Button } from "@/components/ui/button";
import { Kbd } from "@/components/ui/kbd";
import { captureIsWaiting } from "@/store/pixelCaptureSlice";
import type { CaptureStatus } from "@/store/types";

interface Props {
  status: CaptureStatus;
  onDismiss: () => void;
}

export function CaptureStatusPanel({ status, onDismiss }: Props) {
  if (captureIsWaiting(status) || status.phase === "capturing" || status.phase === "saving") return null;
  if (status.phase === "failed") {
    return (
      <div role="alert" className="flex shrink-0 items-center justify-between gap-2 border-b border-destructive/40 px-3 py-2 text-xs">
        <p className="min-w-0 break-words">{status.reason}</p>
        <Button variant="ghost" size="sm" className="shrink-0" onClick={onDismiss}>Закрыть</Button>
      </div>
    );
  }
  return (
    <div className="flex shrink-0 items-center justify-between gap-2 border-b bg-muted/50 px-3 py-2 text-xs">
      <p role="status" aria-label="Состояние захвата">{phaseMessage(status)}</p>
    </div>
  );
}

function phaseMessage(status: CaptureStatus) {
  switch (status.phase) {
    case "selectingSearch": return <>Выберите область поиска иконки. <Kbd>Esc</Kbd> — отмена</>;
    case "selectingTemplate": return <>Выберите образец внутри области поиска. <Kbd>Esc</Kbd> — отмена</>;
    default: return <>Выберите точку на снимке. <Kbd>Esc</Kbd> — отмена</>;
  }
}
