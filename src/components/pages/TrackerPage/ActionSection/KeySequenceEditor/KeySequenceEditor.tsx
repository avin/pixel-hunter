import { Plus } from "lucide-react";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Table,
  TableBody,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { useKeyCodesListQuery } from "@/store/api";
import type { KeyCombo, KeyStep } from "@/store/types";
import type { ActionEditorProps } from "../actionTypes";
import { comboLabel } from "./comboLabel";
import { KeyRecorder } from "./KeyRecorder/KeyRecorder";
import { StepRow } from "./StepRow/StepRow";

type Props = ActionEditorProps<"keySequence">;

const DEFAULT_STEP_DELAY_MS = { min: 100, max: 200 };

export function KeySequenceEditor({ action, onChange }: Props) {
  const keyCodes = useKeyCodesListQuery();
  const [isUnsupportedShown, setIsUnsupportedShown] = useState(false);
  const { steps } = action;

  if (keyCodes.isLoading) {
    return (
      <div role="status" aria-label="Загрузка клавиш" className="flex flex-col gap-2">
        <Skeleton className="h-8" />
        <Skeleton className="h-8 w-40" />
      </div>
    );
  }
  if (!keyCodes.data) {
    return (
      <div role="alert" className="flex items-center gap-2 px-3 py-2">
        <p className="text-sm">Не удалось загрузить список клавиш.</p>
        <Button variant="outline" size="sm" onClick={() => keyCodes.refetch()}>
          Повторить
        </Button>
      </div>
    );
  }

  const keyLabels = new Map(keyCodes.data.map(({ code, label }) => [code, label]));
  const supportedCodes = new Set(keyLabels.keys());

  function save(nextSteps: KeyStep[]) {
    setIsUnsupportedShown(false);
    onChange({ ...action, steps: nextSteps });
  }

  function addStep(combo: KeyCombo) {
    const delayBeforeMs = steps.length > 0 ? { ...DEFAULT_STEP_DELAY_MS } : null;
    save([...steps, { id: crypto.randomUUID(), delayBeforeMs, combo }]);
  }

  function deleteStep(index: number) {
    save(
      steps
        .filter((_, candidate) => candidate !== index)
        .map((step, position) => (position === 0 ? { ...step, delayBeforeMs: null } : step)),
    );
  }

  const showUnsupported = () => setIsUnsupportedShown(true);

  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col">
      {steps.length === 0 ? (
        <div className="min-h-0 flex-1 overflow-auto px-3 py-3">
          <h3 className="text-sm font-medium">Нажатия не настроены</h3>
          <p className="mt-1 text-xs text-muted-foreground">
            Добавьте клавиши и сочетания, которые приложение нажмёт в игре по порядку.
          </p>
        </div>
      ) : (
        <div className="min-h-0 flex-1 overflow-hidden [&>div]:h-full [&>div]:overflow-auto">
          <Table aria-label="Шаги последовательности" className="min-w-[560px] text-xs [&_td]:py-1 [&_th]:h-8">
            <TableHeader className="sticky top-0 z-10 bg-background">
              <TableRow>
                <TableHead className="w-10">№</TableHead>
                <TableHead>Сочетание</TableHead>
                <TableHead className="w-52">Задержка, мс</TableHead>
                <TableHead className="w-36"><span className="sr-only">Команды</span></TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {steps.map((step, index) => (
                <StepRow
                  key={step.id}
                  stepNumber={index + 1}
                  step={step}
                  label={comboLabel(step.combo, keyLabels)}
                  supportedCodes={supportedCodes}
                  onChange={(changed) =>
                    save(steps.map((candidate) => (candidate.id === step.id ? changed : candidate)))
                  }
                  onDelete={() => deleteStep(index)}
                  onUnsupported={showUnsupported}
                />
              ))}
            </TableBody>
          </Table>
        </div>
      )}

      {isUnsupportedShown && (
        <p role="alert" className="shrink-0 px-3 py-2 text-xs text-destructive">
          Эта клавиша не поддерживается. Нажмите другую.
        </p>
      )}

      <div className="shrink-0 border-t pt-2">
        <KeyRecorder
          variant="outline"
          size="sm"
          supportedCodes={supportedCodes}
          onRecord={addStep}
          onUnsupported={showUnsupported}
        >
          <Plus />
          Добавить нажатие
        </KeyRecorder>
      </div>
    </div>
  );
}
