import { createMemoryRouter, RouterProvider } from "react-router-dom";
import { useState } from "react";
import { AppLayout } from "@/components/app/AppLayout/AppLayout";
import { ProfileRoute } from "@/components/app/ProfileRoute/ProfileRoute";
import { ProfilePage } from "@/components/pages/ProfilePage/ProfilePage";
import { ProfilesPage } from "@/components/pages/ProfilesPage/ProfilesPage";
import { TrackerPage } from "@/components/pages/TrackerPage/TrackerPage";
import { PROFILE_ROUTE, TRACKER_SUBROUTE } from "./paths";

export function createAppRouter(initialEntries = ["/"], initialIndex?: number) {
  return createMemoryRouter([
    { element: <AppLayout />, children: [
      { path: "/", element: <ProfilesPage /> },
      { path: PROFILE_ROUTE, element: <ProfileRoute />, children: [
        { index: true, element: <ProfilePage /> },
        { path: TRACKER_SUBROUTE, element: <TrackerPage /> },
      ] },
    ] },
  ], { initialEntries, initialIndex });
}
interface Props { router?: ReturnType<typeof createAppRouter> }
export function AppRouter({ router: suppliedRouter }: Props) {
  const [router] = useState(() => suppliedRouter ?? createAppRouter());
  return <RouterProvider router={router} />;
}
