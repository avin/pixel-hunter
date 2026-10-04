import { MemoryRouter, Route, Routes } from "react-router-dom";
import { AppLayout } from "@/components/app/AppLayout/AppLayout";
import { ProfileRoute } from "@/components/app/ProfileRoute/ProfileRoute";
import { ProfilePage } from "@/components/pages/ProfilePage/ProfilePage";
import { ProfilesPage } from "@/components/pages/ProfilesPage/ProfilesPage";
import { TrackerPage } from "@/components/pages/TrackerPage/TrackerPage";
import { PROFILE_ROUTE, TRACKER_SUBROUTE } from "./paths";

export function AppRouter() {
  return (
    <MemoryRouter initialEntries={["/"]}>
      <Routes>
        <Route element={<AppLayout />}>
          <Route path="/" element={<ProfilesPage />} />
          <Route path={PROFILE_ROUTE} element={<ProfileRoute />}>
            <Route index element={<ProfilePage />} />
            <Route path={TRACKER_SUBROUTE} element={<TrackerPage />} />
          </Route>
        </Route>
      </Routes>
    </MemoryRouter>
  );
}
