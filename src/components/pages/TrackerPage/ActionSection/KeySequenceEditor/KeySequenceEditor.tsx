import { Plus } from "lucide-react";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { useActionUpdateMutation, useKeyCodesListQuery } from "@/store/api";
import type { KeyCombo, KeyStep } from "@/store/types";
import type { ActionEditorProps } from "../actionTypes";
import { comboLabel } from "./comboLabel";
import { KeyRecorder } from "./KeyRecorder/KeyRecorder";
import { StepRow } from "./StepRow/StepRow";

type Props = ActionEditorProps<"keySequence">;

const DEFAULT_STEP_DELAY_MS = { min: 300, max: 600 };

export function KeySequenceEditor({ profileId, trackerId, action }: Props) {
  const keyCodes = useKeyCodesListQuery();
  const [updateAction] = useActionUpdateMutation();
  const [isUnsupportedShown, setIsUnsupportedShown] = useState(false);
  const { steps } = action;

  if (keyCodes.isLoading) {
    return (
      <div role="status" aria-label="Загрузка клавиш" className="flex flex-col gap-2">
        <Skeleton className="h-10" />
        <Skeleton className="h-8 w-40" />
      </div>
    );
  }
  if (!keyCodes.data) {
    return (
      <div role="alert" className="flex flex-col items-start gap-3">
        <p className="text-sm">Не удалось загрузить список клавиш.</p>
        <Button variant="outline" onClick={() => keyCodes.refetch()}>
          Повторить
        </Button>
      </div>
    );
  }

  const keyLabels = new Map(keyCodes.data.map(({ code, label }) => [code, label]));
  const supportedCodes = new Set(keyLabels.keys());

  function save(nextSteps: KeyStep[]) {
    setIsUnsupportedShown(false);
    void updateAction({ profileId, trackerId, action: { ...action, steps: nextSteps } });
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
    <div className="flex flex-col gap-4">
      {steps.length === 0 ? (
        <div className="flex flex-col items-center gap-2 rounded-xl border border-dashed px-6 py-8 text-center">
          <h3 className="font-heading text-base font-medium">Нажатия не настроены</h3>
          <p className="max-w-sm text-sm text-muted-foreground">
            Добавьте клавиши и сочетания, которые приложение нажмёт в игре по порядку.
          </p>
        </div>
      ) : (
        <ol aria-label="Шаги последовательности" className="flex flex-col gap-2">
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
        </ol>
      )}

      {isUnsupportedShown && (
        <p role="alert" className="text-sm text-destructive">
          Эта клавиша не поддерживается. Нажмите другую.
        </p>
      )}

      <div>
        <KeyRecorder
          variant="outline"
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
