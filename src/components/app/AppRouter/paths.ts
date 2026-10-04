export const PROFILE_ROUTE = "/profiles/:profileId";
/** Tracker route relative to `PROFILE_ROUTE`. */
export const TRACKER_SUBROUTE = "trackers/:trackerId";
export const TRACKER_ROUTE = `${PROFILE_ROUTE}/${TRACKER_SUBROUTE}`;

export function profilePath(profileId: string) {
  return `/profiles/${profileId}`;
}

export function trackerPath(profileId: string, trackerId: string) {
  return `/profiles/${profileId}/trackers/${trackerId}`;
}
