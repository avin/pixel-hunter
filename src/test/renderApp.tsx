import { mockConvertFileSrc } from "@tauri-apps/api/mocks";
import { render } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { App } from "@/components/app/App/App";
import { createAppStore } from "@/store";
import { createFakeBackend, type FakeBackend } from "./fakeBackend";

export function renderApp(backend: FakeBackend = createFakeBackend()) {
  backend.install();
  mockConvertFileSrc("windows");
  const user = userEvent.setup();
  render(<App store={createAppStore()} />);
  return { user, backend };
}
