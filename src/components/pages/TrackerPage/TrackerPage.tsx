import { Trash2 } from "lucide-react";
import { useState } from "react";
import { Link, Navigate, useParams } from "react-router-dom";
import { profilePath } from "@/components/app/AppRouter/paths";
import { useWorkspaceNavigation } from "@/components/app/AppLayout/workspaceNavigation";
import { Button, buttonVariants } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { useListProfilesQuery } from "@/store/api";
import { RunToggle } from "@/components/pages/ProfilePage/RunToggle/RunToggle";
import { DeleteTrackerDialog } from "@/components/pages/ProfilePage/DeleteTrackerDialog/DeleteTrackerDialog";
import { ActionSection } from "./ActionSection/ActionSection";
import { PixelsSection } from "./PixelsSection/PixelsSection";
import { TrackerForm } from "./TrackerForm/TrackerForm";
import { TrackerSplit } from "./TrackerSplit/TrackerSplit";

export function TrackerPage() {
  const { profileId = "", trackerId = "" } = useParams();
  const [isDeleteDialogOpen, setIsDeleteDialogOpen] = useState(false);
  const [isDeleted, setIsDeleted] = useState(false);
  const { isLeaving } = useWorkspaceNavigation();
  const { profile, tracker, isLoading, isError, refetch } = useListProfilesQuery(
    undefined,
    {
      selectFromResult: ({ data, isLoading, isError }) => {
        const profile = data?.find(({ id }) => id === profileId);
        return {
          profile,
          tracker: profile?.trackers.find(({ id }) => id === trackerId),
          isLoading,
          isError,
        };
      },
    },
  );

  if (isDeleted) {
    return <Navigate to={profilePath(profileId)} replace />;
  }
  if (isLoading) {
    return (
      <div role="status" aria-label="Загрузка отслеживания" className="flex max-w-md flex-col gap-2 p-3">
        <Skeleton className="h-8" />
        <Skeleton className="h-16" />
        <Skeleton className="h-8" />
      </div>
    );
  }
  if (isError) {
    return (
      <div role="alert" className="flex flex-col items-start gap-3 p-3">
        <p className="text-sm">Не удалось загрузить отслеживание.</p>
        <Button variant="outline" onClick={() => refetch()}>
          Повторить
        </Button>
      </div>
    );
  }
  if (!profile || !tracker) {
    return (
      <div className="flex flex-col items-start gap-3 p-3">
        <p className="text-sm">Отслеживание не найдено.</p>
        <Link
          to={profilePath(profileId)}
          className={buttonVariants({ variant: "outline" })}
        >
          К профилю
        </Link>
      </div>
    );
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <div className="flex shrink-0 items-center justify-between gap-3 border-b px-3 py-2">
        <span className="truncate font-medium">Настройки отслеживания</span>
        <div className="flex shrink-0 items-center gap-1">
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label={`Удалить отслеживание «${tracker.name}»`}
            disabled={isLeaving}
            onClick={() => setIsDeleteDialogOpen(true)}
          >
            <Trash2 />
          </Button>
          <Separator orientation="vertical" className="mx-2" aria-hidden="true" />
          <RunToggle profileId={profileId} tracker={tracker} />
        </div>
      </div>
      <TrackerForm key={`${profileId}/${tracker.id}`} profileId={profileId} tracker={tracker} />
      <TrackerSplit
        pixels={
          <PixelsSection
            profileId={profileId}
            processName={profile.processName}
            tracker={tracker}
          />
        }
        action={<ActionSection profileId={profileId} tracker={tracker} />}
      />
      {isDeleteDialogOpen && (
        <DeleteTrackerDialog
          profileId={profileId}
          tracker={tracker}
          onClose={() => setIsDeleteDialogOpen(false)}
          onDeleted={() => setIsDeleted(true)}
        />
      )}
    </div>
  );
}
