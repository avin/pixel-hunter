import { ArrowLeft } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Link, matchPath, Outlet, useLocation } from "react-router-dom";
import { screenFromPath } from "@/components/app/AppRouter/initialScreen";
import {
  PROFILE_ROUTE,
  profilePath,
  TRACKER_ROUTE,
} from "@/components/app/AppRouter/paths";
import { buttonVariants } from "@/components/ui/button";
import { useListProfilesQuery, useSetLastScreenMutation } from "@/store/api";
import { GameWindowStatus } from "@/components/pages/ProfilePage/GameWindowStatus/GameWindowStatus";
import { WorkControls } from "./WorkControls/WorkControls";
import { WorkspaceSidebar } from "./WorkspaceSidebar/WorkspaceSidebar";
import { WorkspaceNavigationContext } from "./workspaceNavigation";

export function AppLayout() {
  const { title, backTo, profile, trackerId } = useHeader();
  const [isLeaving, setIsLeaving] = useState(false);
  useRememberScreen();

  return (
    <WorkspaceNavigationContext value={{ isLeaving, setIsLeaving }}>
      <div className="flex h-svh overflow-hidden text-sm">
        <WorkspaceSidebar
          profileId={profile?.id}
          trackerId={trackerId}
          isLeaving={isLeaving}
        />
        <div className="flex min-w-0 flex-1 flex-col">
          <header
            inert={isLeaving}
            className="flex h-10 shrink-0 items-center gap-2 border-b px-3"
          >
            {backTo && (
              <Link
                to={backTo}
                className={buttonVariants({ variant: "ghost", size: "sm" })}
              >
                <ArrowLeft />
                Назад
              </Link>
            )}
            <h1 className="truncate font-heading text-sm font-medium">
              {title}
            </h1>
          </header>
          <main className="flex min-h-0 flex-1 flex-col overflow-hidden">
            <Outlet />
          </main>
          <footer className="flex min-h-8 shrink-0 items-center gap-3 border-t px-3 text-xs">
            <WorkControls />
            {profile ? (
              <>
                <span className="shrink-0 text-muted-foreground">
                  Запущено:{" "}
                  {profile.trackers.filter(({ isRunning }) => isRunning).length}
                  /{profile.trackers.length}
                </span>
                <GameWindowStatus
                  profileId={profile.id}
                  processName={profile.processName}
                />
              </>
            ) : (
              <span className="text-muted-foreground">
                Выберите профиль для работы
              </span>
            )}
          </footer>
        </div>
      </div>
    </WorkspaceNavigationContext>
  );
}

function useHeader() {
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
      profile,
      trackerId: trackerMatch.params.trackerId,
    };
  }
  if (profileMatch) {
    return {
      title: profile?.name ?? "Профиль",
      backTo: "/",
      profile,
      trackerId: undefined,
    };
  }
  return {
    title: "Профили",
    backTo: undefined,
    profile: undefined,
    trackerId: undefined,
  };
}

/** Writes the open screen after the restored route, never on the first frame. */
function useRememberScreen() {
  const { pathname } = useLocation();
  const [setLastScreen] = useSetLastScreenMutation();
  const isFirst = useRef(true);

  useEffect(() => {
    if (isFirst.current) {
      isFirst.current = false;
      return;
    }
    void setLastScreen(screenFromPath(pathname));
  }, [pathname, setLastScreen]);
}
