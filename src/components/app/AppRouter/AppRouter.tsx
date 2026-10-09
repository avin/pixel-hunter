import { createMemoryRouter, RouterProvider } from "react-router-dom";
import { useRef } from "react";
import { AppLayout } from "@/components/app/AppLayout/AppLayout";
import { ProfileRoute } from "@/components/app/ProfileRoute/ProfileRoute";
import { ProfilePage } from "@/components/pages/ProfilePage/ProfilePage";
import { ProfilesPage } from "@/components/pages/ProfilesPage/ProfilesPage";
import { TrackerPage } from "@/components/pages/TrackerPage/TrackerPage";
import { useGetSettingsQuery, useListProfilesQuery } from "@/store/api";
import { initialPath } from "./initialScreen";
import { PROFILE_ROUTE, TRACKER_SUBROUTE } from "./paths";

export function createAppRouter(initialEntries = ["/"], initialIndex?: number) {
  return createMemoryRouter(
    [
      {
        element: <AppLayout />,
        children: [
          { path: "/", element: <ProfilesPage /> },
          {
            path: PROFILE_ROUTE,
            element: <ProfileRoute />,
            children: [
              { index: true, element: <ProfilePage /> },
              { path: TRACKER_SUBROUTE, element: <TrackerPage /> },
            ],
          },
        ],
      },
    ],
    { initialEntries, initialIndex },
  );
}

interface Props {
  router?: ReturnType<typeof createAppRouter>;
}

export function AppRouter({ router: suppliedRouter }: Props) {
  if (suppliedRouter) return <RouterProvider router={suppliedRouter} />;
  return <RestoredRouter />;
}

/** Mounts the router only after settings and profiles have settled, so `/` is not saved first. */
function RestoredRouter() {
  const settings = useGetSettingsQuery();
  const profiles = useListProfilesQuery();
  const routerRef = useRef<ReturnType<typeof createAppRouter> | null>(null);
  const settingsSettled = settings.isSuccess || settings.isError;
  const profilesSettled = profiles.isSuccess || profiles.isError;

  if (!routerRef.current && settingsSettled && profilesSettled) {
    routerRef.current = createAppRouter([
      initialPath(settings.data?.lastScreen, profiles.data ?? []),
    ]);
  }

  if (!routerRef.current) return null;
  return <RouterProvider router={routerRef.current} />;
}
