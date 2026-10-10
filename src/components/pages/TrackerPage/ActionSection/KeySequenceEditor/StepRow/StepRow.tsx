import { Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Kbd } from "@/components/ui/kbd";
import { useKeyCapture } from "@/hooks/useKeyCapture";
import { TableCell, TableRow } from "@/components/ui/table";
import type { KeyStep } from "@/store/types";
import { KeyRecorder } from "../KeyRecorder/KeyRecorder";
import { StepDelay } from "./StepDelay/StepDelay";

interface Props {
  stepNumber: number;
  step: KeyStep;
  label: string;
  supportedCodes: ReadonlySet<string>;
  onChange: (step: KeyStep) => void;
  onDelete: () => void;
  onUnsupported: () => void;
}

export function StepRow({
  stepNumber,
  step,
  label,
  supportedCodes,
  onChange,
  onDelete,
  onUnsupported,
}: Props) {
  const capture = useKeyCapture();
  return (
    <TableRow>
      <TableCell className="font-mono text-muted-foreground tabular-nums">{stepNumber}.</TableCell>
      <TableCell>
        <Kbd>{label}</Kbd>
      </TableCell>
      <TableCell>
        {step.delayBeforeMs ? (
          <StepDelay
            label={`Задержка перед шагом ${stepNumber}`}
            range={step.delayBeforeMs}
            onSave={(delayBeforeMs) => onChange({ ...step, delayBeforeMs })}
          />
        ) : (
          <span aria-label="Без задержки" className="text-muted-foreground">—</span>
        )}
      </TableCell>
      <TableCell>
        <StepDelay
          label={`Удержание шага ${stepNumber}`}
          range={step.holdMs}
          onSave={(holdMs) => onChange({ ...step, holdMs })}
        />
      </TableCell>
      <TableCell>
        <div className="flex items-center justify-end gap-1">
          <KeyRecorder
            variant="ghost"
            size="sm"
            aria-label={`Перезаписать шаг ${stepNumber}`}
            supportedCodes={supportedCodes}
            onRecord={(combo) => onChange({ ...step, combo })}
            onUnsupported={onUnsupported}
            onPrepareRecording={capture.onPrepareRecording}
            onRecordingChange={capture.onRecordingChange}
          >
            Перезаписать
          </KeyRecorder>
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label={`Удалить шаг ${stepNumber}`}
            onClick={onDelete}
          >
            <Trash2 />
          </Button>
        </div>
      </TableCell>
    </TableRow>
  );
}
