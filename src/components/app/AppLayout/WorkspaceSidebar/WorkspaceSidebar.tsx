import {
  ChevronDown,
  ChevronRight,
  List,
  Plus,
} from "lucide-react";
import { cn } from "cn";
import { useState } from "react";
import { Link } from "react-router-dom";
import { profilePath, trackerPath } from "@/components/app/AppRouter/paths";
import { ProfileDialog } from "@/components/pages/ProfilesPage/ProfileDialog/ProfileDialog";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { useListProfilesQuery } from "@/store/api";
import type { Profile } from "@/store/types";

interface Props {
  profileId?: string;
  trackerId?: string;
  isLeaving: boolean;
}

const linkClass =
  "flex min-w-0 flex-1 items-center gap-2 rounded-sm px-2 py-1.5 outline-none hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring";

export function WorkspaceSidebar({ profileId, trackerId, isLeaving }: Props) {
  const { data: profiles, isLoading, isError } = useListProfilesQuery();
  const [expandedProfiles, setExpandedProfiles] = useState<
    Record<string, boolean>
  >({});
  const [editedProfile, setEditedProfile] = useState<{ profile?: Profile }>();

  return (
    <aside className="flex w-48 shrink-0 flex-col border-r bg-sidebar text-sidebar-foreground md:w-52">
      <div className="flex h-10 shrink-0 items-center justify-between border-b px-2">
        <span className="font-medium">Pixel Hunter</span>
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label="Создать профиль в навигации"
          disabled={isLeaving}
          onClick={() => setEditedProfile({})}
        >
          <Plus />
        </Button>
      </div>
      <nav
        aria-label="Профили и отслеживания"
        className="min-h-0 flex-1 overflow-y-auto p-2"
      >
        <Link
          to="/"
          aria-current={!profileId ? "page" : undefined}
          aria-disabled={isLeaving || undefined}
          onClick={(event) => isLeaving && event.preventDefault()}
          className={cn(
            linkClass,
            !profileId && "bg-sidebar-accent font-medium",
          )}
        >
          <List className="size-4 shrink-0" />
          Все профили
        </Link>
        {isLoading && <Skeleton className="mt-2 h-8" />}
        {isError && (
          <p className="px-2 py-2 text-muted-foreground">Профили недоступны</p>
        )}
        {profiles?.map((profile) => {
          const isExpanded =
            expandedProfiles[profile.id] ?? profile.id === profileId;
          return (
            <div key={profile.id} className="mt-1">
              <div className="flex min-w-0 items-center">
                <Button
                  variant="ghost"
                  size="icon-sm"
                  aria-label={`${isExpanded ? "Свернуть" : "Раскрыть"} профиль «${profile.name}»`}
                  aria-expanded={isExpanded}
                  onClick={() =>
                    setExpandedProfiles((current) => ({
                      ...current,
                      [profile.id]: !isExpanded,
                    }))
                  }
                >
                  {isExpanded ? <ChevronDown /> : <ChevronRight />}
                </Button>
                <Link
                  to={profilePath(profile.id)}
                  title={profile.name}
                  aria-label={`Открыть профиль «${profile.name}»`}
                  aria-current={
                    profile.id === profileId && !trackerId ? "page" : undefined
                  }
                  aria-disabled={isLeaving || undefined}
                  onClick={(event) => isLeaving && event.preventDefault()}
                  className={cn(
                    linkClass,
                    profile.id === profileId &&
                      !trackerId &&
                      "bg-sidebar-accent font-medium",
                  )}
                >
                  <span className="truncate">{profile.name}</span>
                </Link>
              </div>
              {isExpanded && (
                <div className="ml-4 border-l pl-2">
                  {profile.trackers.map((tracker) => (
                    <div key={tracker.id} className="flex min-w-0 items-center">
                      <Link
                        to={trackerPath(profile.id, tracker.id)}
                        title={`${tracker.name} (${tracker.isRunning ? "запущено" : "остановлено"})`}
                        aria-label={`Открыть отслеживание «${tracker.name}»`}
                        aria-current={
                          profile.id === profileId && tracker.id === trackerId
                            ? "page"
                            : undefined
                        }
                        aria-disabled={isLeaving || undefined}
                        onClick={(event) => isLeaving && event.preventDefault()}
                        className={cn(
                          linkClass,
                          profile.id === profileId &&
                            tracker.id === trackerId &&
                            "bg-sidebar-accent font-medium",
                        )}
                      >
                        <span className="truncate">{tracker.name}</span>
                        {tracker.isRunning && (
                          <span
                            className="ml-auto shrink-0 text-success"
                            aria-label="Запущено"
                          >
                            ●
                          </span>
                        )}
                      </Link>
                    </div>
                  ))}
                </div>
              )}
            </div>
          );
        })}
      </nav>
      {editedProfile && (
        <ProfileDialog
          profile={editedProfile.profile}
          onClose={() => setEditedProfile(undefined)}
        />
      )}
    </aside>
  );
}
