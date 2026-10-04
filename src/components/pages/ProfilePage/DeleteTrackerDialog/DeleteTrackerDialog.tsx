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
import { useDeleteTrackerMutation } from "@/store/api";
import type { Tracker } from "@/store/types";

interface Props {
  profileId: string;
  tracker: Tracker;
  onClose: () => void;
}

export function DeleteTrackerDialog({ profileId, tracker, onClose }: Props) {
  const [deleteTracker, { isLoading }] = useDeleteTrackerMutation();

  async function handleDelete() {
    const result = await deleteTracker({ profileId, trackerId: tracker.id });
    if (!("error" in result)) {
      onClose();
    }
  }

  return (
    <AlertDialog open onOpenChange={(isOpen) => !isOpen && onClose()}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Удалить отслеживание «{tracker.name}»?</AlertDialogTitle>
          <AlertDialogDescription>
            Отслеживание и вся его конфигурация будут удалены без возможности
            восстановления.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Отмена</AlertDialogCancel>
          <AlertDialogAction
            variant="destructive"
            disabled={isLoading}
            onClick={handleDelete}
          >
            Удалить
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
