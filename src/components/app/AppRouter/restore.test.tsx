import { screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import {
  createFakeBackend,
  profileFixture,
  trackerFixture,
} from "@/test/fakeBackend";
import { renderRestoredApp } from "@/test/renderApp";

const health = trackerFixture({ id: "health", name: "Здоровье", isRunning: true });
const diablo = profileFixture({
  id: "diablo",
  name: "Diablo",
  trackers: [health],
});

describe("restored screen", () => {
  it("does not save the list route before settings arrive", async () => {
    const backend = createFakeBackend({ profiles: [diablo] });
    const release = backend.hold("settings_get");
    renderRestoredApp(backend);

    expect(screen.queryByRole("heading", { name: "Профили" })).not.toBeInTheDocument();
    expect(backend.callsOf("settings_set_last_screen")).toEqual([]);

    release();
    expect(await screen.findByRole("heading", { level: 1, name: "Профили" })).toBeInTheDocument();
    expect(backend.callsOf("settings_set_last_screen")).toEqual([]);
    expect(backend.callsOf("engine_activate")).toEqual([]);
  });

  it("opens the profile list when nothing was saved", async () => {
    const backend = createFakeBackend({ profiles: [diablo] });
    renderRestoredApp(backend);

    expect(await screen.findByRole("heading", { level: 1, name: "Профили" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Diablo" })).toBeInTheDocument();
    expect(backend.callsOf("engine_activate")).toEqual([]);
  });

  it("opens a saved profile and runs its trackers", async () => {
    const backend = createFakeBackend({
      profiles: [diablo],
      settings: {
        pauseHotkey: null,
        lastScreen: { kind: "profile", profileId: "diablo" },
      },
    });
    renderRestoredApp(backend);

    expect(await screen.findByRole("heading", { level: 1, name: "Diablo" })).toBeInTheDocument();
    expect(await screen.findByRole("status", { name: "Работает" })).toBeInTheDocument();
    await waitFor(() =>
      expect(backend.callsOf("engine_activate")).toEqual([{ profileId: "diablo", epoch: 1 }]),
    );
    expect(backend.state.profiles[0].trackers[0].isRunning).toBe(true);
  });

  it("opens the profile when a tracker editor was saved", async () => {
    const backend = createFakeBackend({
      profiles: [diablo],
      settings: {
        pauseHotkey: null,
        lastScreen: { kind: "editor", profileId: "diablo", trackerId: "health" },
      },
    });
    renderRestoredApp(backend);

    expect(await screen.findByRole("heading", { level: 1, name: "Diablo" })).toBeInTheDocument();
    expect(screen.queryByRole("heading", { level: 1, name: "Здоровье" })).not.toBeInTheDocument();
    await waitFor(() =>
      expect(backend.callsOf("engine_activate")).toEqual([{ profileId: "diablo", epoch: 1 }]),
    );
  });

  it("opens the list when the saved profile is gone", async () => {
    const backend = createFakeBackend({
      profiles: [diablo],
      settings: {
        pauseHotkey: null,
        lastScreen: { kind: "profile", profileId: "missing" },
      },
    });
    renderRestoredApp(backend);

    expect(await screen.findByRole("heading", { level: 1, name: "Профили" })).toBeInTheDocument();
    expect(backend.callsOf("engine_activate")).toEqual([]);
  });

  it("opens the profile when the saved tracker is gone", async () => {
    const backend = createFakeBackend({
      profiles: [diablo],
      settings: {
        pauseHotkey: null,
        lastScreen: { kind: "editor", profileId: "diablo", trackerId: "missing" },
      },
    });
    renderRestoredApp(backend);

    expect(await screen.findByRole("heading", { level: 1, name: "Diablo" })).toBeInTheDocument();
    await waitFor(() =>
      expect(backend.callsOf("engine_activate")).toEqual([{ profileId: "diablo", epoch: 1 }]),
    );
  });

  it("opens the list when settings cannot be read", async () => {
    const backend = createFakeBackend({
      profiles: [diablo],
      settings: {
        pauseHotkey: null,
        lastScreen: { kind: "profile", profileId: "diablo" },
      },
    });
    backend.failOn("settings_get", "Не удалось прочитать настройки");
    renderRestoredApp(backend);

    expect(await screen.findByRole("heading", { level: 1, name: "Профили" })).toBeInTheDocument();
    expect(backend.callsOf("engine_activate")).toEqual([]);
  });

  it("saves the screen as the user moves and leaves the engine idle on the list", async () => {
    const { user, backend } = renderRestoredApp(createFakeBackend({ profiles: [diablo] }));
    await user.click(await screen.findByRole("link", { name: "Diablo" }));
    await waitFor(() =>
      expect(backend.callsOf("settings_set_last_screen")).toContainEqual({
        lastScreen: { kind: "profile", profileId: "diablo" },
      }),
    );
    await waitFor(() =>
      expect(backend.callsOf("engine_activate")).toEqual([{ profileId: "diablo", epoch: 1 }]),
    );

    await user.click(screen.getByRole("link", { name: "Открыть отслеживание «Здоровье»" }));
    await waitFor(() =>
      expect(backend.callsOf("settings_set_last_screen")).toContainEqual({
        lastScreen: { kind: "editor", profileId: "diablo", trackerId: "health" },
      }),
    );

    await user.click(screen.getByRole("link", { name: "Все профили" }));
    await waitFor(() =>
      expect(backend.callsOf("settings_set_last_screen")).toContainEqual({
        lastScreen: { kind: "profiles" },
      }),
    );
    await waitFor(() => expect(backend.callsOf("engine_deactivate")).toHaveLength(1));
    expect(backend.callsOf("engine_activate")).toEqual([{ profileId: "diablo", epoch: 1 }]);
    expect(backend.state.profiles[0].trackers[0].isRunning).toBe(true);
  });
});
