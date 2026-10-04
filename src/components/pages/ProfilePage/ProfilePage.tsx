import { Plus, Trash2 } from "lucide-react";
import { useState } from "react";
import { Link, useParams } from "react-router-dom";
import { trackerPath } from "@/components/app/AppRouter/paths";
import { Button, buttonVariants } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { useListProfilesQuery } from "@/store/api";
import type { Tracker } from "@/store/types";
import { CreateTrackerDialog } from "./CreateTrackerDialog/CreateTrackerDialog";
import { DeleteTrackerDialog } from "./DeleteTrackerDialog/DeleteTrackerDialog";

export function ProfilePage() {
  const { profileId = "" } = useParams();
  const { profile, isLoading, isError, refetch } = useListProfilesQuery(
    undefined,
    {
      selectFromResult: ({ data, isLoading, isError }) => ({
        profile: data?.find(({ id }) => id === profileId),
        isLoading,
        isError,
      }),
    },
  );
  const [isCreateDialogOpen, setIsCreateDialogOpen] = useState(false);
  const [trackerToDelete, setTrackerToDelete] = useState<Tracker>();

  if (isLoading) {
    return (
      <div role="status" aria-label="Загрузка профиля" className="flex flex-col gap-2 p-6">
        <Skeleton className="h-5 w-48" />
        <Skeleton className="h-10" />
        <Skeleton className="h-10" />
      </div>
    );
  }
  if (isError) {
    return (
      <div role="alert" className="flex flex-col items-start gap-3 p-6">
        <p className="text-sm">Не удалось загрузить профиль.</p>
        <Button variant="outline" onClick={() => refetch()}>
          Повторить
        </Button>
      </div>
    );
  }
  if (!profile) {
    return (
      <div className="flex flex-col items-start gap-3 p-6">
        <p className="text-sm">Профиль не найден.</p>
        <Link to="/" className={buttonVariants({ variant: "outline" })}>
          К списку профилей
        </Link>
      </div>
    );
  }

  const openCreateDialog = () => setIsCreateDialogOpen(true);

  return (
    <div className="flex flex-col gap-4 p-6">
      <div className="flex items-center justify-between gap-4">
        <p className="text-sm text-muted-foreground">
          Процесс: <span className="text-foreground">{profile.processName}</span>
        </p>
        {profile.trackers.length > 0 && (
          <Button onClick={openCreateDialog}>
            <Plus />
            Создать отслеживание
          </Button>
        )}
      </div>

      {profile.trackers.length === 0 ? (
        <div className="flex flex-col items-center gap-3 rounded-xl border border-dashed px-6 py-12 text-center">
          <h2 className="font-heading text-base font-medium">
            Отслеживаний пока нет
          </h2>
          <p className="max-w-sm text-sm text-muted-foreground">
            Отслеживание следит за пикселями на экране игры и выполняет действие,
            когда условия совпадают.
          </p>
          <Button onClick={openCreateDialog}>
            <Plus />
            Создать отслеживание
          </Button>
        </div>
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Имя</TableHead>
              <TableHead>Задержка перед действием</TableHead>
              <TableHead>Пауза до возобновления</TableHead>
              <TableHead className="w-0">
                <span className="sr-only">Действия</span>
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {profile.trackers.map((tracker) => (
              <TableRow key={tracker.id}>
                <TableCell>
                  <Link
                    to={trackerPath(profile.id, tracker.id)}
                    className="font-medium underline-offset-4 outline-none hover:underline focus-visible:underline focus-visible:ring-3 focus-visible:ring-ring/50"
                  >
                    {tracker.name}
                  </Link>
                </TableCell>
                <TableCell className="text-muted-foreground">
                  {tracker.preActionDelayMs.min}–{tracker.preActionDelayMs.max} мс
                </TableCell>
                <TableCell className="text-muted-foreground">
                  {tracker.resumeDelayMs} мс
                </TableCell>
                <TableCell>
                  <div className="flex justify-end">
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      aria-label={`Удалить отслеживание «${tracker.name}»`}
                      onClick={() => setTrackerToDelete(tracker)}
                    >
                      <Trash2 />
                    </Button>
                  </div>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}

      {isCreateDialogOpen && (
        <CreateTrackerDialog
          profileId={profile.id}
          onClose={() => setIsCreateDialogOpen(false)}
        />
      )}
      {trackerToDelete && (
        <DeleteTrackerDialog
          profileId={profile.id}
          tracker={trackerToDelete}
          onClose={() => setTrackerToDelete(undefined)}
        />
      )}
    </div>
  );
}
