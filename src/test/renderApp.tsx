import { mockConvertFileSrc } from "@tauri-apps/api/mocks";
import { render } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { App } from "@/components/app/App/App";
import { createAppRouter } from "@/components/app/AppRouter/AppRouter";
import { createAppStore } from "@/store";
import { createFakeBackend, type FakeBackend } from "./fakeBackend";

export function renderApp(backend: FakeBackend = createFakeBackend(), initialEntries = ["/"], initialIndex?: number) {
  backend.install();
  mockConvertFileSrc("windows");
  const user = userEvent.setup();
  const router = createAppRouter(initialEntries, initialIndex);
  render(<App store={createAppStore()} router={router} />);
  return { user, backend, router };
}

/** Starts the app the way production does: the router appears only after settings and profiles load. */
export function renderRestoredApp(backend: FakeBackend = createFakeBackend()) {
  backend.install();
  mockConvertFileSrc("windows");
  const user = userEvent.setup();
  render(<App store={createAppStore()} />);
  return { user, backend };
}
