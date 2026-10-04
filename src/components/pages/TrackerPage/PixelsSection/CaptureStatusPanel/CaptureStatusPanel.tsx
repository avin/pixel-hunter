import { Button } from "@/components/ui/button";
import type { CaptureStatus } from "@/store/types";

interface Props {
  status: CaptureStatus;
  processName: string;
  onCancel: () => void;
  onDismiss: () => void;
}

export function CaptureStatusPanel({ status, processName, onCancel, onDismiss }: Props) {
  if (status.phase === "timeout" || status.phase === "failed") {
    return (
      <div
        role="alert"
        className="flex items-center justify-between gap-3 rounded-lg border border-destructive/40 px-4 py-3 text-sm"
      >
        <p>
          {status.phase === "timeout"
            ? "Окно игры не стало активным за 10 секунд. Пиксель не добавлен."
            : status.reason}
        </p>
        <Button variant="ghost" size="sm" onClick={onDismiss}>
          Закрыть
        </Button>
      </div>
    );
  }

  return (
    <div className="flex items-center justify-between gap-3 rounded-lg border bg-muted/50 px-4 py-3 text-sm">
      <div className="flex items-baseline gap-2">
        <p role="status">{phaseMessage(status, processName)}</p>
        {"remainingMs" in status && (
          <span className="font-medium tabular-nums">
            {Math.ceil(status.remainingMs / 1000)} с
          </span>
        )}
      </div>
      <Button variant="outline" size="sm" onClick={onCancel}>
        Отмена
      </Button>
    </div>
  );
}

function phaseMessage(status: CaptureStatus, processName: string) {
  switch (status.phase) {
    case "waitingForGame":
      return `Переключитесь в окно игры ${processName}`;
    case "countdown":
      return "Снимок экрана через";
    default:
      return "Выберите точку на снимке. Esc — отмена";
  }
}
