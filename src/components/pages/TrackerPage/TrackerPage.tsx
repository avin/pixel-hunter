import { Link, useParams } from "react-router-dom";
import { profilePath } from "@/components/app/AppRouter/paths";
import { Button, buttonVariants } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { useListProfilesQuery } from "@/store/api";
import { TrackerForm } from "./TrackerForm/TrackerForm";

export function TrackerPage() {
  const { profileId = "", trackerId = "" } = useParams();
  const { tracker, isLoading, isError, refetch } = useListProfilesQuery(
    undefined,
    {
      selectFromResult: ({ data, isLoading, isError }) => ({
        tracker: data
          ?.find(({ id }) => id === profileId)
          ?.trackers.find(({ id }) => id === trackerId),
        isLoading,
        isError,
      }),
    },
  );

  if (isLoading) {
    return (
      <div role="status" aria-label="Загрузка отслеживания" className="flex max-w-md flex-col gap-4 p-6">
        <Skeleton className="h-8" />
        <Skeleton className="h-16" />
        <Skeleton className="h-8" />
      </div>
    );
  }
  if (isError) {
    return (
      <div role="alert" className="flex flex-col items-start gap-3 p-6">
        <p className="text-sm">Не удалось загрузить отслеживание.</p>
        <Button variant="outline" onClick={() => refetch()}>
          Повторить
        </Button>
      </div>
    );
  }
  if (!tracker) {
    return (
      <div className="flex flex-col items-start gap-3 p-6">
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

  return <TrackerForm key={tracker.id} profileId={profileId} tracker={tracker} />;
}
