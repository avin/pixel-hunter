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
import { useDeleteProfileMutation } from "@/store/api";
import type { Profile } from "@/store/types";

interface Props {
  profile: Profile;
  onClose: () => void;
  onDeleted?: () => void;
}

export function DeleteProfileDialog({ profile, onClose, onDeleted }: Props) {
  const [deleteProfile, { isLoading }] = useDeleteProfileMutation();

  async function handleDelete() {
    const result = await deleteProfile({ id: profile.id });
    if (!("error" in result)) {
      onClose();
      onDeleted?.();
    }
  }

  return (
    <AlertDialog open onOpenChange={(isOpen) => !isOpen && onClose()}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Удалить профиль «{profile.name}»?</AlertDialogTitle>
          <AlertDialogDescription>
            Профиль и все его отслеживания будут удалены без возможности
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
