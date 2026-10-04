import { useId } from "react";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useActionUpdateMutation } from "@/store/api";
import type { ActionType, Tracker } from "@/store/types";
import { actionTypes } from "./actionTypes";

interface Props {
  profileId: string;
  tracker: Tracker;
}

const TYPE_LABELS = Object.fromEntries(
  Object.entries(actionTypes).map(([type, { label }]) => [type, label]),
) as Record<ActionType, string>;

export function ActionSection({ profileId, tracker }: Props) {
  const [updateAction] = useActionUpdateMutation();
  const headingId = useId();
  const typeLabelId = useId();
  const { action } = tracker;
  const { Editor } = actionTypes[action.type];

  return (
    <section aria-labelledby={headingId} className="flex flex-col gap-4 border-t p-6">
      <h2 id={headingId} className="font-heading text-base font-medium">
        Действие
      </h2>

      <div className="flex items-center gap-3">
        <span id={typeLabelId} className="text-sm font-medium">
          Тип
        </span>
        <Select
          items={TYPE_LABELS}
          value={action.type}
          onValueChange={(type) => {
            if (type) {
              void updateAction({
                profileId,
                trackerId: tracker.id,
                action: actionTypes[type].createDefault(),
              });
            }
          }}
        >
          <SelectTrigger aria-labelledby={typeLabelId}>
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

      <Editor profileId={profileId} trackerId={tracker.id} action={action} />
    </section>
  );
}
