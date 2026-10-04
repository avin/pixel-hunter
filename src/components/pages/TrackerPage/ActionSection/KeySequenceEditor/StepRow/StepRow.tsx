import { Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
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
  return (
    <TableRow>
      <TableCell className="font-mono text-muted-foreground tabular-nums">{stepNumber}.</TableCell>
      <TableCell>
        <kbd className="font-mono text-xs">{label}</kbd>
      </TableCell>
      <TableCell>
        {step.delayBeforeMs ? (
          <StepDelay
            stepNumber={stepNumber}
            delay={step.delayBeforeMs}
            onSave={(delayBeforeMs) => onChange({ ...step, delayBeforeMs })}
          />
        ) : (
          <span aria-label="Без задержки" className="text-muted-foreground">—</span>
        )}
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
