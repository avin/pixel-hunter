import { Zap } from "lucide-react";
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
    <section aria-label="Действие" className="flex min-h-0 flex-col overflow-hidden border">
      <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-b px-3 py-2">
        <div className="flex items-center gap-2">
          <Label id={typeLabelId} htmlFor={typeId} className="text-sm font-semibold">
            <Zap aria-hidden="true" className="size-4 shrink-0" />
            Действие
          </Label>
          <Select
            items={TYPE_LABELS}
            value={action.type}
            onValueChange={(type) => {
              if (type) changeType(type);
            }}
          >
            <SelectTrigger id={typeId} size="sm" aria-labelledby={typeLabelId}>
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
      <div className="flex min-h-0 flex-1 flex-col gap-2 overflow-hidden p-2">
        {edit?.error && <div role="alert" className="flex items-center gap-2 text-xs text-destructive"><span>Не удалось сохранить действие: {edit.error}</span><Button variant="outline" size="sm" onClick={() => void dispatch(saveDesiredAction(profileId, tracker.id))}>Повторить сохранение</Button></div>}
        <ActionEditor profileId={profileId} trackerId={tracker.id} action={action} onChange={updateAction} />
      </div>
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
