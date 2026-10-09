import { matchPath } from "react-router-dom";
import type { LastScreen, Profile } from "@/store/types";
import { PROFILE_ROUTE, TRACKER_ROUTE, profilePath, trackerPath } from "./paths";

/** Route to open on startup. An unknown profile or a failed read falls back to the list. */
export function initialPath(
  lastScreen: LastScreen | undefined,
  profiles: readonly Profile[],
) {
  if (!lastScreen || lastScreen.kind === "profiles") return "/";
  const profile = profiles.find((item) => item.id === lastScreen.profileId);
  if (!profile) return "/";
  if (lastScreen.kind === "profile") return profilePath(profile.id);
  const tracker = profile.trackers.find((item) => item.id === lastScreen.trackerId);
  if (!tracker) return profilePath(profile.id);
  return trackerPath(profile.id, tracker.id);
}

export function screenFromPath(pathname: string): LastScreen {
  const tracker = matchPath(TRACKER_ROUTE, pathname);
  if (tracker?.params.profileId && tracker.params.trackerId) {
    return {
      kind: "editor",
      profileId: tracker.params.profileId,
      trackerId: tracker.params.trackerId,
    };
  }
  const profile = matchPath(PROFILE_ROUTE, pathname);
  if (profile?.params.profileId) {
    return { kind: "profile", profileId: profile.params.profileId };
  }
  return { kind: "profiles" };
}
