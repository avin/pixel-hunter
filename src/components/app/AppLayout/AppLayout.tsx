import { Outlet } from "react-router-dom";
import { AppSidebar } from "./AppSidebar/AppSidebar";

export function AppLayout() {
  return (
    <div className="flex h-svh overflow-hidden">
      <AppSidebar />
      <main className="flex min-w-0 flex-1 flex-col overflow-auto">
        <Outlet />
      </main>
    </div>
  );
}
