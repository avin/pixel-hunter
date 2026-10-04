import { Pencil, Plus, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { Link, useLocation, useNavigate, useParams } from "react-router-dom";
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
import { ProfileDialog } from "@/components/pages/ProfilesPage/ProfileDialog/ProfileDialog";
import { DeleteProfileDialog } from "@/components/pages/ProfilesPage/DeleteProfileDialog/DeleteProfileDialog";
import type { Tracker } from "@/store/types";
import { CreateTrackerDialog } from "./CreateTrackerDialog/CreateTrackerDialog";
import { DeleteTrackerDialog } from "./DeleteTrackerDialog/DeleteTrackerDialog";
import { RunToggle } from "./RunToggle/RunToggle";

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
  const [isEditProfileOpen, setIsEditProfileOpen] = useState(false);
  const [isDeleteProfileOpen, setIsDeleteProfileOpen] = useState(false);
  const [isCreateDialogOpen, setIsCreateDialogOpen] = useState(false);
  const [trackerToDelete, setTrackerToDelete] = useState<Tracker>();
  const location = useLocation();
  const navigate = useNavigate();
  const deleteTrackerId = (
    location.state as { deleteTrackerId?: string } | null
  )?.deleteTrackerId;

  useEffect(() => {
    if (!deleteTrackerId || !profile) return;
    setTrackerToDelete(
      profile.trackers.find(({ id }) => id === deleteTrackerId),
    );
    navigate(location.pathname, { replace: true, state: null });
  }, [deleteTrackerId, profile, navigate, location.pathname]);

  if (isLoading) {
    return (
      <div
        role="status"
        aria-label="Загрузка профиля"
        className="flex flex-col gap-2 p-3"
      >
        <Skeleton className="h-5 w-48" />
        <Skeleton className="h-10" />
        <Skeleton className="h-10" />
      </div>
    );
  }
  if (isError) {
    return (
      <div role="alert" className="flex flex-col items-start gap-2 p-3">
        <p className="text-sm">Не удалось загрузить профиль.</p>
        <Button variant="outline" onClick={() => refetch()}>
          Повторить
        </Button>
      </div>
    );
  }
  if (!profile) {
    return (
      <div className="flex flex-col items-start gap-2 p-3">
        <p className="text-sm">Профиль не найден.</p>
        <Link to="/" className={buttonVariants({ variant: "outline" })}>
          К списку профилей
        </Link>
      </div>
    );
  }

  const openCreateDialog = () => setIsCreateDialogOpen(true);

  return (
    <div className="flex flex-col min-h-0 flex-1 gap-2 p-3">
      <div className="flex shrink-0 items-center justify-between gap-2">
        <p className="min-w-0 truncate text-sm text-muted-foreground">
          Процесс:{" "}
          <span className="text-foreground">{profile.processName}</span>
        </p>
        <div className="flex shrink-0 items-center gap-1">
          <Button
            variant="outline"
            size="sm"
            aria-label={`Изменить профиль «${profile.name}»`}
            onClick={() => setIsEditProfileOpen(true)}
          >
            <Pencil />
            Изменить
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label={`Удалить профиль «${profile.name}»`}
            onClick={() => setIsDeleteProfileOpen(true)}
          >
            <Trash2 />
          </Button>
          {profile.trackers.length > 0 && (
            <Button onClick={openCreateDialog}>
              <Plus />
              Создать отслеживание
            </Button>
          )}
        </div>
      </div>

      {profile.trackers.length === 0 ? (
        <div className="flex flex-col items-start gap-2 border border-dashed p-3">
          <h2 className="font-heading text-base font-medium">
            Отслеживаний пока нет
          </h2>
          <p className="max-w-sm text-sm text-muted-foreground">
            Отслеживание следит за пикселями на экране игры и выполняет
            действие, когда условия совпадают.
          </p>
          <Button onClick={openCreateDialog}>
            <Plus />
            Создать отслеживание
          </Button>
        </div>
      ) : (
        <div className="min-h-0 flex-1 overflow-auto border">
          <Table>
            <TableHeader className="sticky top-0 z-10 bg-background">
              <TableRow>
                <TableHead>Имя</TableHead>
                <TableHead>Состояние</TableHead>
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
                  <TableCell>
                    <RunToggle profileId={profile.id} tracker={tracker} />
                  </TableCell>
                  <TableCell className="font-mono text-muted-foreground">
                    {tracker.preActionDelayMs.min}–
                    {tracker.preActionDelayMs.max} мс
                  </TableCell>
                  <TableCell className="font-mono text-muted-foreground">
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
        </div>
      )}

      {isEditProfileOpen && (
        <ProfileDialog profile={profile} onClose={() => setIsEditProfileOpen(false)} />
      )}
      {isDeleteProfileOpen && (
        <DeleteProfileDialog
          profile={profile}
          onClose={() => setIsDeleteProfileOpen(false)}
          onDeleted={() => navigate("/")}
        />
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
