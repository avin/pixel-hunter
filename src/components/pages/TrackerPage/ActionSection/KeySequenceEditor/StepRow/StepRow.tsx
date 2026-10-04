import { Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
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
    <li className="flex flex-col gap-2 rounded-lg border px-3 py-2">
      {step.delayBeforeMs && (
        <StepDelay
          stepNumber={stepNumber}
          delay={step.delayBeforeMs}
          onSave={(delayBeforeMs) => onChange({ ...step, delayBeforeMs })}
        />
      )}
      <div className="flex items-center gap-3">
        <span className="w-5 text-sm text-muted-foreground tabular-nums">{stepNumber}.</span>
        <kbd className="rounded-md border bg-muted px-2 py-0.5 font-mono text-sm">{label}</kbd>
        <div className="ml-auto flex items-center gap-1">
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
      </div>
    </li>
  );
}
