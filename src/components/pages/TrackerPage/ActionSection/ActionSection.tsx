import { useId, useState } from "react";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
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
import { ActionEditor } from "./ActionEditor";
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
  const [pendingType, setPendingType] = useState<ActionType | null>(null);
  function updateAction(next: Action) {
    dispatch(setDesiredAction({ key, action: next }));
    void dispatch(saveDesiredAction(profileId, tracker.id));
  }
  function applyType(type: ActionType) {
    setPendingType(null);
    updateAction({ ...actionTypes[type].createDefault(), sound: action.sound });
  }
  function changeType(type: ActionType) {
    if (type === action.type) return;
    if (action.type === "keySequence" && action.steps.length > 0) {
      setPendingType(type);
      return;
    }
    applyType(type);
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
            if (type) changeType(type);
          }}
        >
          <SelectTrigger id={typeId} aria-labelledby={typeLabelId}>
            <span className="grid">
              <span aria-hidden className="invisible col-start-1 row-start-1 grid">
                {Object.values(TYPE_LABELS).map((label) => (
                  <span key={label} className="col-start-1 row-start-1 whitespace-nowrap">
                    {label}
                  </span>
                ))}
              </span>
              <SelectValue className="col-start-1 row-start-1" />
            </span>
          </SelectTrigger>
          <SelectContent align="start" alignItemWithTrigger={false} className="w-max min-w-(--anchor-width)">
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
      <ActionEditor profileId={profileId} trackerId={tracker.id} action={action} onChange={updateAction} />
      {pendingType && (
        <AlertDialog open onOpenChange={(isOpen) => { if (!isOpen) setPendingType(null); }}>
          <AlertDialogContent>
            <AlertDialogHeader>
              <AlertDialogTitle>Сменить тип действия?</AlertDialogTitle>
              <AlertDialogDescription>Шаги нажатий будут удалены.</AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter>
              <AlertDialogCancel>Отмена</AlertDialogCancel>
              <AlertDialogAction onClick={() => applyType(pendingType)}>Сменить</AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      )}
    </section>
  );
}
