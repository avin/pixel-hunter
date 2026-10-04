import { MemoryRouter, Route, Routes } from "react-router-dom";
import { AppLayout } from "@/components/app/AppLayout/AppLayout";
import { AboutPage } from "@/components/pages/AboutPage/AboutPage";
import { HomePage } from "@/components/pages/HomePage/HomePage";
import { SettingsPage } from "@/components/pages/SettingsPage/SettingsPage";

export function AppRouter() {
  return (
    <MemoryRouter initialEntries={["/"]}>
      <Routes>
        <Route element={<AppLayout />}>
          <Route path="/" element={<HomePage />} />
          <Route path="/settings" element={<SettingsPage />} />
          <Route path="/about" element={<AboutPage />} />
        </Route>
      </Routes>
    </MemoryRouter>
  );
}
