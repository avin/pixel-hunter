import { Button } from "@/components/ui/button";
import { Kbd } from "@/components/ui/kbd";
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
        className="flex shrink-0 items-center justify-between gap-2 border-b border-destructive/40 px-3 py-2 text-xs"
      >
        <p className="min-w-0 break-words">
          {status.phase === "timeout"
            ? "Окно игры не стало активным за 10 секунд. Элемент не добавлен."
            : status.reason}
        </p>
        <Button variant="ghost" size="sm" className="shrink-0" onClick={onDismiss}>
          Закрыть
        </Button>
      </div>
    );
  }

  return (
    <div className="flex shrink-0 items-center justify-between gap-2 border-b bg-muted/50 px-3 py-2 text-xs">
      <div className="flex min-w-0 flex-wrap items-baseline gap-x-2 gap-y-1">
        <p role="status" aria-label="Состояние захвата">{phaseMessage(status, processName)}</p>
        {status.phase === "waitingForGame" && (
          <span className="shrink-0 font-mono tabular-nums">
            {formatRemaining(status.remainingMs)}
          </span>
        )}
      </div>
      <Button variant="outline" size="sm" className="shrink-0" onClick={onCancel}>
        Отмена
      </Button>
    </div>
  );
}

function formatRemaining(remainingMs: number) {
  if (remainingMs < 1000) return `${remainingMs} мс`;
  return `${Math.ceil(remainingMs / 1000)} с`;
}

function phaseMessage(status: CaptureStatus, processName: string) {
  switch (status.phase) {
    case "waitingForGame":
      return `Переключитесь в окно игры ${processName}`;
    case "countdown":
      return "Делаем снимок экрана...";
    case "selectingSearch":
      return <>Выберите область поиска иконки. <Kbd>Esc</Kbd> — отмена</>;
    case "selectingTemplate":
      return <>Выберите образец внутри области поиска. <Kbd>Esc</Kbd> — отмена</>;
    default:
      return <>Выберите точку на снимке. <Kbd>Esc</Kbd> — отмена</>;
  }
}
