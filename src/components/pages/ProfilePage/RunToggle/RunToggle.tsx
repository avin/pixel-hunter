import { Play, Square } from "lucide-react";
import { useId } from "react";
import { Button } from "@/components/ui/button";
import { useTrackerSetRunningMutation } from "@/store/api";
import { actionIsEmpty, type Tracker } from "@/store/types";

interface Props {
  profileId: string;
  tracker: Tracker;
}

function missingPartHint(tracker: Tracker) {
  if (tracker.trigger === "hotkey") {
    if (!tracker.hotkey) return "Назначьте сочетание, чтобы запустить";
  } else if (tracker.pixels.length === 0) {
    return "Добавьте пиксель, чтобы запустить";
  }
  if (actionIsEmpty(tracker.action)) return "Добавьте шаг действия, чтобы запустить";
  return undefined;
}

export function RunToggle({ profileId, tracker }: Props) {
  const hintId = useId();
  const [setRunning, { isLoading }] = useTrackerSetRunningMutation();
  const hint = missingPartHint(tracker);
  const isRunning = tracker.isRunning && !hint;

  return (
    <div className="flex items-center gap-2">
      <Button
        variant={isRunning ? "destructive" : "outline"}
        size="sm"
        className="w-24"
        aria-pressed={isRunning}
        aria-describedby={hint ? hintId : undefined}
        disabled={Boolean(hint) || isLoading}
        onClick={() =>
          void setRunning({ profileId, trackerId: tracker.id, isRunning: !isRunning })
        }
      >
        {isRunning ? <Square /> : <Play />}
        {isRunning ? "Стоп" : "Старт"} <span className="sr-only">отслеживания «{tracker.name}»</span>
      </Button>
      {hint && (
        <span id={hintId} className="text-xs text-muted-foreground">
          {hint}
        </span>
      )}
    </div>
  );
}
