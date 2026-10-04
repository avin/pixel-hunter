export const PROFILE_ROUTE = "/profiles/:profileId";
export const TRACKER_ROUTE = "/profiles/:profileId/trackers/:trackerId";

export function profilePath(profileId: string) {
  return `/profiles/${profileId}`;
}

export function trackerPath(profileId: string, trackerId: string) {
  return `/profiles/${profileId}/trackers/${trackerId}`;
}
