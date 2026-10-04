import { ArrowLeft } from "lucide-react";
import { Link, matchPath, Outlet, useLocation } from "react-router-dom";
import {
  PROFILE_ROUTE,
  profilePath,
  TRACKER_ROUTE,
} from "@/components/app/AppRouter/paths";
import { buttonVariants } from "@/components/ui/button";
import { useListProfilesQuery } from "@/store/api";

export function AppLayout() {
  const { title, backTo } = useHeader();

  return (
    <div className="flex h-svh flex-col overflow-hidden">
      <header className="flex h-14 shrink-0 items-center gap-3 border-b px-4">
        {backTo && (
          <Link
            to={backTo}
            className={buttonVariants({ variant: "ghost", size: "sm" })}
          >
            <ArrowLeft />
            Назад
          </Link>
        )}
        <h1 className="truncate font-heading text-base font-medium">{title}</h1>
      </header>
      <main className="flex min-h-0 flex-1 flex-col overflow-auto">
        <Outlet />
      </main>
    </div>
  );
}

function useHeader(): { title: string; backTo?: string } {
  const { pathname } = useLocation();
  const { data: profiles } = useListProfilesQuery();

  const trackerMatch = matchPath(TRACKER_ROUTE, pathname);
  const profileMatch = matchPath(PROFILE_ROUTE, pathname);
  const profileId =
    trackerMatch?.params.profileId ?? profileMatch?.params.profileId;
  const profile = profiles?.find(({ id }) => id === profileId);

  if (trackerMatch && profileId) {
    const tracker = profile?.trackers.find(
      ({ id }) => id === trackerMatch.params.trackerId,
    );
    return {
      title: tracker?.name ?? "Отслеживание",
      backTo: profilePath(profileId),
    };
  }
  if (profileMatch) {
    return { title: profile?.name ?? "Профиль", backTo: "/" };
  }
  return { title: "Профили" };
}
