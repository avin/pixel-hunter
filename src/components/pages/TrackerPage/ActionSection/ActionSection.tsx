import { useId } from "react";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { actionEditKey, saveDesiredAction, setDesiredAction } from "@/store/actionEditsSlice";
import { useAppDispatch, useAppSelector } from "@/store/hooks";
import type { Action, ActionType, Tracker } from "@/store/types";
import { actionTypes } from "./actionTypes";
import { SoundSettings } from "./SoundSettings/SoundSettings";

interface Props {
  profileId: string;
  tracker: Tracker;
}

const TYPE_LABELS = Object.fromEntries(
  Object.entries(actionTypes).map(([type, { label }]) => [type, label]),
) as Record<ActionType, string>;

export function ActionSection({ profileId, tracker }: Props) {
  const dispatch = useAppDispatch();
  const key = actionEditKey(profileId, tracker.id);
  const edit = useAppSelector((state) => state.actionEdits[key]);
  const headingId = useId();
  const typeLabelId = useId();
  const typeId = useId();
  const action = edit?.action ?? tracker.action;
  const { Editor } = actionTypes[action.type];
  function updateAction(next: Action) {
    dispatch(setDesiredAction({ key, action: next }));
    void dispatch(saveDesiredAction(profileId, tracker.id));
  }

  return (
    <section aria-labelledby={headingId} className="flex min-h-0 flex-col gap-2 overflow-hidden border p-2">
      <h2 id={headingId} className="font-heading text-sm font-medium">
        Действие
      </h2>

      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-2">
        <Label id={typeLabelId} htmlFor={typeId}>Тип</Label>
        <Select
          items={TYPE_LABELS}
          value={action.type}
          onValueChange={(type) => {
            if (type) {
              updateAction({ ...actionTypes[type].createDefault(), sound: action.sound });
            }
          }}
        >
          <SelectTrigger id={typeId} aria-labelledby={typeLabelId}>
            <SelectValue />
          </SelectTrigger>
          <SelectContent alignItemWithTrigger={false}>
            {Object.entries(TYPE_LABELS).map(([type, label]) => (
              <SelectItem key={type} value={type}>
                {label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        </div>
        <SoundSettings key={key} sound={action.sound} onChange={(sound) => updateAction({ ...action, sound })} />
      </div>
      {edit?.error && <div role="alert" className="flex items-center gap-2 text-xs text-destructive"><span>Не удалось сохранить действие: {edit.error}</span><Button variant="outline" size="sm" onClick={() => void dispatch(saveDesiredAction(profileId, tracker.id))}>Повторить сохранение</Button></div>}
      <Editor profileId={profileId} trackerId={tracker.id} action={action} onChange={updateAction} />
    </section>
  );
}
