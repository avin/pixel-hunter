import { MemoryRouter, Route, Routes } from "react-router-dom";
import { AppLayout } from "@/components/app/AppLayout/AppLayout";
import { ProfilePage } from "@/components/pages/ProfilePage/ProfilePage";
import { ProfilesPage } from "@/components/pages/ProfilesPage/ProfilesPage";
import { TrackerPage } from "@/components/pages/TrackerPage/TrackerPage";
import { PROFILE_ROUTE, TRACKER_ROUTE } from "./paths";

export function AppRouter() {
  return (
    <MemoryRouter initialEntries={["/"]}>
      <Routes>
        <Route element={<AppLayout />}>
          <Route path="/" element={<ProfilesPage />} />
          <Route path={PROFILE_ROUTE} element={<ProfilePage />} />
          <Route path={TRACKER_ROUTE} element={<TrackerPage />} />
        </Route>
      </Routes>
    </MemoryRouter>
  );
}
