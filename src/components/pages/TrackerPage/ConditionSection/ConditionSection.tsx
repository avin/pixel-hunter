import { ListFilter } from "lucide-react";
import { useId, useState, type ReactNode } from "react";
import { comboLabel } from "@/components/pages/TrackerPage/ActionSection/KeySequenceEditor/comboLabel";
import { KeyRecorder } from "@/components/pages/TrackerPage/ActionSection/KeySequenceEditor/KeyRecorder/KeyRecorder";
import { Button } from "@/components/ui/button";
import { Kbd } from "@/components/ui/kbd";
import { Skeleton } from "@/components/ui/skeleton";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useKeyCapture } from "@/hooks/useKeyCapture";
import {
  useKeyCodesListQuery,
  useSetTrackerHotkeyMutation,
  useSetTrackerTriggerMutation,
} from "@/store/api";
import type { KeyCombo, Tracker, Trigger } from "@/store/types";

interface Props {
  profileId: string;
  tracker: Tracker;
}

const TRIGGER_LABELS: Record<Trigger, string> = {
  pixels: "Визуальное",
  hotkey: "Хоткей",
};

function commandError(error: unknown) {
  if (error && typeof error === "object" && "message" in error && typeof error.message === "string") {
    return error.message;
  }
  return "Не удалось сохранить сочетание";
}

export function ConditionSelect({ profileId, tracker }: Props) {
  const labelId = useId();
  const selectId = useId();
  const [setTrigger] = useSetTrackerTriggerMutation();
  const [error, setError] = useState<string>();

  return (
    <div className="flex min-w-0 flex-wrap items-center gap-2">
      <Label id={labelId} htmlFor={selectId} className="text-sm font-semibold">
        <ListFilter aria-hidden="true" className="size-4 shrink-0" />
        Условие
      </Label>
      <Select
        items={TRIGGER_LABELS}
        value={tracker.trigger}
        onValueChange={(trigger) => {
          if (!trigger || trigger === tracker.trigger) return;
          setError(undefined);
          void setTrigger({ profileId, trackerId: tracker.id, trigger }).unwrap().catch((caught: unknown) => {
            setError(commandError(caught));
          });
        }}
      >
        <SelectTrigger id={selectId} size="sm" aria-labelledby={labelId}>
          <SelectValue />
        </SelectTrigger>
        <SelectContent align="start">
          {(Object.entries(TRIGGER_LABELS) as [Trigger, string][]).map(([value, label]) => (
            <SelectItem key={value} value={value}>
              {label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      {error && <p role="alert" className="text-xs text-destructive">{error}</p>}
    </div>
  );
}

interface HeaderProps extends Props {
  children?: ReactNode;
}

export function ConditionHeader({ profileId, tracker, children }: HeaderProps) {
  return (
    <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-b px-3 py-2">
      <ConditionSelect profileId={profileId} tracker={tracker} />
      {children}
    </div>
  );
}

export function HotkeySection({ profileId, tracker }: Props) {
  const keyCodes = useKeyCodesListQuery();
  const [setHotkey] = useSetTrackerHotkeyMutation();
  const capture = useKeyCapture();
  const [error, setError] = useState<string>();
  const labels = new Map((keyCodes.data ?? []).map(({ code, label }) => [code, label]));

  function save(hotkey: KeyCombo | null) {
    setError(undefined);
    void setHotkey({ profileId, trackerId: tracker.id, hotkey }).unwrap().catch((caught: unknown) => {
      setError(commandError(caught));
    });
  }

  return (
    <section aria-label="Хоткей" className="flex min-h-0 min-w-0 flex-1 flex-col border">
      <ConditionHeader profileId={profileId} tracker={tracker} />
      <div className="flex min-h-0 flex-col gap-3 overflow-auto p-3">
        <p className="text-sm">
          {tracker.hotkey ? (
            <>
              Сейчас назначено{" "}
              <Kbd>{comboLabel(tracker.hotkey, labels)}</Kbd>.
            </>
          ) : (
            "Сочетание не назначено."
          )}
        </p>
        {keyCodes.isLoading ? (
          <Skeleton className="h-8 w-28" aria-label="Загрузка сочетания" />
        ) : keyCodes.data ? (
          <div className="flex flex-wrap items-center gap-2">
            <KeyRecorder
              variant="outline"
              size="sm"
              supportedCodes={new Set(labels.keys())}
              onRecord={(combo) => save(combo)}
              onUnsupported={() => setError("Эта клавиша не поддерживается")}
              onPrepareRecording={capture.onPrepareRecording}
              onRecordingChange={capture.onRecordingChange}
              recordingLabel="Нажмите нужное сочетание..."
            >
              Назначить
            </KeyRecorder>
            {tracker.hotkey && (
              <Button variant="outline" size="sm" onClick={() => save(null)}>
                Сбросить сочетание
              </Button>
            )}
          </div>
        ) : (
          <p role="alert" className="text-sm">Не удалось загрузить список клавиш.</p>
        )}
        {error && <p role="alert" className="text-xs text-destructive">{error}</p>}
      </div>
    </section>
  );
}
