import { screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import {
  createFakeBackend,
  profileFixture,
  trackerFixture,
} from "@/test/fakeBackend";
import { renderApp } from "@/test/renderApp";

const profile = profileFixture({
  id: "diablo",
  name: "Diablo",
  trackers: [trackerFixture({ name: "Здоровье" })],
});

async function openProfile() {
  const rendered = renderApp(createFakeBackend({ profiles: [profile] }));
  await rendered.user.click(await screen.findByRole("link", { name: "Diablo" }));
  await screen.findByRole("link", { name: "Здоровье" });
  return rendered;
}

describe("ProfileRoute", () => {
  it("activates the engine for the opened profile", async () => {
    const { backend } = await openProfile();

    await waitFor(() =>
      expect(backend.callsOf("engine_activate")).toEqual([{ profileId: "diablo" }]),
    );
    expect(backend.callsOf("engine_deactivate")).toEqual([]);
  });

  it("keeps the engine running between the profile and its trackers", async () => {
    const { user, backend } = await openProfile();

    await user.click(screen.getByRole("link", { name: "Здоровье" }));
    await screen.findByRole("heading", { level: 1, name: "Здоровье" });
    await user.click(screen.getByRole("link", { name: "Назад" }));
    await screen.findByRole("heading", { level: 1, name: "Diablo" });

    expect(backend.callsOf("engine_deactivate")).toEqual([]);
    expect(backend.callsOf("engine_activate")).toHaveLength(1);
  });

  it("deactivates the engine on return to the profile list", async () => {
    const { user, backend } = await openProfile();

    await user.click(screen.getByRole("link", { name: "Назад" }));
    await screen.findByRole("heading", { level: 1, name: "Профили" });

    await waitFor(() => expect(backend.callsOf("engine_deactivate")).toHaveLength(1));
  });
});
