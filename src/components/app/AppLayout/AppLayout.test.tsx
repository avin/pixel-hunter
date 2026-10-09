import { emit } from "@tauri-apps/api/event";
import { act, cleanup, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { WORK_PAUSE_EVENT } from "@/hooks/useWorkEvents";
import {
  createFakeBackend,
  keyStepFixture,
  pixelFixture,
  profileFixture,
  trackerFixture,
} from "@/test/fakeBackend";
import { renderApp } from "@/test/renderApp";

const profile = profileFixture({
  id: "diablo",
  name: "Diablo",
  trackers: [trackerFixture({ id: "health", name: "Здоровье" })],
});

describe("workspace navigation", () => {
  it("expands a profile without selecting it or activating its engine", async () => {
    const { user, backend } = renderApp(
      createFakeBackend({ profiles: [profile] }),
    );
    await user.click(
      await screen.findByRole("button", { name: "Раскрыть профиль «Diablo»" }),
    );
    expect(
      screen.getByRole("link", { name: "Открыть отслеживание «Здоровье»" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Все профили" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(screen.queryByRole("link", { name: "Обзор" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Изменить профиль «Diablo» в навигации" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Удалить профиль «Diablo» в навигации" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Создать отслеживание в профиле «Diablo»" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Удалить отслеживание/ })).not.toBeInTheDocument();
    expect(backend.callsOf("engine_activate")).toEqual([]);

    await user.click(
      screen.getByRole("button", { name: "Свернуть профиль «Diablo»" }),
    );
    expect(
      screen.queryByRole("link", { name: "Открыть отслеживание «Здоровье»" }),
    ).not.toBeInTheDocument();
  });

  it("opens a tracker directly and marks the completed route", async () => {
    const { user, backend } = renderApp(
      createFakeBackend({ profiles: [profile] }),
    );
    await user.click(
      await screen.findByRole("button", { name: "Раскрыть профиль «Diablo»" }),
    );
    await user.click(
      screen.getByRole("link", { name: "Открыть отслеживание «Здоровье»" }),
    );
    expect(
      await screen.findByRole("heading", { level: 1, name: "Здоровье" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("link", { name: "Открыть отслеживание «Здоровье»" }),
    ).toHaveAttribute("aria-current", "page");
    expect(screen.getByText(/Ожидание окна игры/)).toBeInTheDocument();
    await waitFor(() =>
      expect(backend.callsOf("engine_activate")).toEqual([
        { profileId: "diablo", epoch: 1 },
      ]),
    );

    await user.click(screen.getByRole("link", { name: "Все профили" }));
    expect(
      await screen.findByRole("heading", { level: 1, name: "Профили" }),
    ).toBeInTheDocument();
    await waitFor(() =>
      expect(backend.callsOf("engine_deactivate")).toHaveLength(1),
    );
  });

  it("asks for tracker deletion in its editor and keeps it on cancellation", async () => {
    const { user, backend } = renderApp(
      createFakeBackend({ profiles: [profile] }),
      ["/profiles/diablo/trackers/health"],
    );
    await user.click(
      await screen.findByRole("button", {
        name: "Удалить отслеживание «Здоровье»",
      }),
    );
    expect(
      await screen.findByRole("alertdialog", {
        name: "Удалить отслеживание «Здоровье»?",
      }),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Отмена" }));
    expect(
      screen.getByRole("heading", { level: 1, name: "Здоровье" }),
    ).toBeInTheDocument();
    expect(backend.state.profiles[0].trackers).toHaveLength(1);
    expect(
      screen.getByRole("link", { name: "Открыть отслеживание «Здоровье»" }),
    ).toBeInTheDocument();
  });

  it("deletes a tracker with an invalid draft and returns to its profile", async () => {
    const { user, backend } = renderApp(
      createFakeBackend({ profiles: [profile] }),
      ["/profiles/diablo/trackers/health"],
    );
    const name = await screen.findByRole("textbox", { name: "Имя" });
    await user.clear(name);
    await user.click(screen.getByRole("button", { name: "Удалить отслеживание «Здоровье»" }));
    expect(
      await screen.findByRole("alertdialog", { name: "Удалить отслеживание «Здоровье»?" }),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Удалить" }));
    expect(
      await screen.findByRole("heading", { level: 1, name: "Diablo" }),
    ).toBeInTheDocument();
    expect(backend.state.profiles[0].trackers).toHaveLength(0);
    expect(backend.callsOf("tracker_update")).toEqual([]);
  });
});

const complete = trackerFixture({
  id: "health",
  name: "Здоровье",
  pixels: [pixelFixture()],
  action: {
    type: "keySequence",
    steps: [keyStepFixture()],
    sound: { id: null, volumePercent: 50 },
  },
});

const savedHotkey = {
  pauseHotkey: { modifiers: ["ctrl" as const], key: "KeyQ" },
  lastScreen: { kind: "profiles" as const },
};

describe("work pause", () => {
  it("waits for the global hotkey to be released before recording", async () => {
    const backend = createFakeBackend({ settings: savedHotkey });
    const release = backend.hold("pause_hotkey_begin");
    const { user } = renderApp(backend);
    await user.click(await screen.findByRole("button", { name: "Настроить паузу" }));
    const dialog = await screen.findByRole("dialog", { name: "Сочетание для паузы" });
    await user.click(await within(dialog).findByRole("button", { name: "Назначить" }));

    expect(within(dialog).queryByRole("button", { name: "Нажмите нужное сочетание..." })).not.toBeInTheDocument();
    await user.keyboard("w");
    expect(backend.callsOf("settings_set_pause_hotkey")).toEqual([]);

    await act(() => release());
    expect(await within(dialog).findByRole("button", { name: "Нажмите нужное сочетание..." })).toBeInTheDocument();
    await user.keyboard("w");
    await waitFor(() => expect(backend.state.settings.pauseHotkey).toEqual({ modifiers: [], key: "KeyW" }));
  });

  it("restores the hotkey when the dialog closes during preparation", async () => {
    const backend = createFakeBackend({ settings: savedHotkey });
    const release = backend.hold("pause_hotkey_begin");
    const { user } = renderApp(backend);
    await user.click(await screen.findByRole("button", { name: "Настроить паузу" }));
    const dialog = await screen.findByRole("dialog", { name: "Сочетание для паузы" });
    await user.click(await within(dialog).findByRole("button", { name: "Назначить" }));
    await user.click(within(dialog).getByRole("button", { name: "Закрыть" }));
    expect(backend.callsOf("pause_hotkey_cancel")).toEqual([]);

    await act(() => release());
    await waitFor(() => expect(backend.callsOf("pause_hotkey_cancel")).toHaveLength(1));
    expect(backend.callsOf("settings_set_pause_hotkey")).toEqual([]);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("cancels preparation on blur and allows another recording", async () => {
    const backend = createFakeBackend({ settings: savedHotkey });
    const release = backend.hold("pause_hotkey_begin");
    const { user } = renderApp(backend);
    await user.click(await screen.findByRole("button", { name: "Настроить паузу" }));
    const dialog = await screen.findByRole("dialog", { name: "Сочетание для паузы" });
    await user.click(await within(dialog).findByRole("button", { name: "Назначить" }));
    expect(within(dialog).getByRole("button", { name: "Подготовка…" })).toHaveAttribute("aria-busy", "true");
    await user.tab();

    await act(() => release());
    await waitFor(() => expect(backend.callsOf("pause_hotkey_cancel")).toHaveLength(1));
    expect(within(dialog).getByRole("button", { name: "Назначить" })).toHaveAttribute("aria-pressed", "false");
    await user.click(within(dialog).getByRole("button", { name: "Назначить" }));
    expect(await within(dialog).findByRole("button", { name: "Нажмите нужное сочетание..." })).toBeInTheDocument();
    await user.keyboard("w");
    await waitFor(() => expect(backend.state.settings.pauseHotkey).toEqual({ modifiers: [], key: "KeyW" }));
  });

  it("keeps recording off when preparation fails", async () => {
    const backend = createFakeBackend({ settings: savedHotkey });
    backend.failOn("pause_hotkey_begin", "Не удалось снять сочетание");
    const { user } = renderApp(backend);
    await user.click(await screen.findByRole("button", { name: "Настроить паузу" }));
    const dialog = await screen.findByRole("dialog", { name: "Сочетание для паузы" });
    await user.click(await within(dialog).findByRole("button", { name: "Назначить" }));
    expect(await screen.findByText("Не удалось снять сочетание")).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Назначить" })).toHaveAttribute("aria-pressed", "false");
    await user.keyboard("w");
    expect(backend.callsOf("settings_set_pause_hotkey")).toEqual([]);
  });

  it("shows the same pause from the event and from the toggle", async () => {
    const { user, backend } = renderApp(createFakeBackend({ profiles: [profile] }));

    expect(await screen.findByRole("status", { name: "Работает" })).toBeInTheDocument();
    await act(() => emit(WORK_PAUSE_EVENT, { paused: true }));
    expect(screen.getByRole("status", { name: "На паузе" })).toBeInTheDocument();
    await act(() => emit(WORK_PAUSE_EVENT, { paused: false }));
    expect(screen.getByRole("status", { name: "Работает" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Пауза" }));
    expect(await screen.findByRole("status", { name: "На паузе" })).toBeInTheDocument();
    expect(backend.state.profiles[0].trackers[0].isRunning).toBe(false);
    expect(backend.callsOf("tracker_set_running")).toEqual([]);
  });

  it("starts and stops a tracker while the work stays paused", async () => {
    const { user, backend } = renderApp(
      createFakeBackend({
        profiles: [profileFixture({ id: "diablo", name: "Diablo", trackers: [complete] })],
      }),
      ["/profiles/diablo"],
    );
    await user.click(await screen.findByRole("button", { name: "Пауза" }));
    expect(await screen.findByRole("status", { name: "На паузе" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Старт отслеживания «Здоровье»" }));
    expect(await screen.findByRole("button", { name: "Стоп отслеживания «Здоровье»" })).toBeInTheDocument();
    expect(backend.state.profiles[0].trackers[0].isRunning).toBe(true);
    expect(screen.getByRole("status", { name: "На паузе" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Стоп отслеживания «Здоровье»" }));
    expect(await screen.findByRole("button", { name: "Старт отслеживания «Здоровье»" })).toBeInTheDocument();
    expect(backend.state.profiles[0].trackers[0].isRunning).toBe(false);
    expect(screen.getByRole("status", { name: "На паузе" })).toBeInTheDocument();
  });

  it("assigns a combo without pausing and keeps the previous one when assignment fails", async () => {
    const backend = createFakeBackend({
      profiles: [profile],
      settings: savedHotkey,
    });
    const { user } = renderApp(backend);

    await user.click(await screen.findByRole("button", { name: "Настроить паузу" }));
    const dialog = await screen.findByRole("dialog", { name: "Сочетание для паузы" });
    await user.click(within(dialog).getByRole("button", { name: "Назначить" }));
    expect(
      within(dialog).getByRole("button", { name: "Нажмите нужное сочетание..." }),
    ).toBeInTheDocument();
    await user.keyboard("w");
    expect(backend.callsOf("work_pause_toggle")).toEqual([]);
    await waitFor(() =>
      expect(backend.state.settings.pauseHotkey).toEqual({ modifiers: [], key: "KeyW" }),
    );
    expect(await within(dialog).findByRole("button", { name: "Назначить" })).toBeInTheDocument();
    expect(within(dialog).getByText(/Сейчас назначено/)).toBeInTheDocument();
    expect(within(dialog).getByText("W")).toBeInTheDocument();

    backend.failOn("settings_set_pause_hotkey", "Система не может занять это сочетание");
    await user.click(within(dialog).getByRole("button", { name: "Назначить" }));
    await user.keyboard("q");
    expect(await screen.findByText("Система не может занять это сочетание")).toBeInTheDocument();
    expect(backend.state.settings.pauseHotkey).toEqual({ modifiers: [], key: "KeyW" });

    await user.click(within(dialog).getByRole("button", { name: "Назначить" }));
    await user.keyboard("{Meta}");
    expect(await within(dialog).findByRole("alert")).toHaveTextContent("Эта клавиша не поддерживается");
    expect(backend.state.settings.pauseHotkey).toEqual({ modifiers: [], key: "KeyW" });

    await user.click(within(dialog).getByRole("button", { name: "Сбросить сочетание" }));
    expect(await within(dialog).findByText("Сочетание не назначено.")).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Назначить" })).toBeInTheDocument();
    expect(backend.state.settings.pauseHotkey).toBeNull();

    await user.click(within(dialog).getByRole("button", { name: "Закрыть" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.getByRole("status", { name: "Работает" })).toBeInTheDocument();
    expect(backend.callsOf("work_pause_toggle")).toEqual([]);
  });

  it("captures keys while the pause shortcut is recorded", async () => {
    const backend = createFakeBackend({ settings: savedHotkey });
    const { user } = renderApp(backend);
    await user.click(await screen.findByRole("button", { name: "Настроить паузу" }));
    const dialog = await screen.findByRole("dialog", { name: "Сочетание для паузы" });

    await user.click(await within(dialog).findByRole("button", { name: "Назначить" }));
    await within(dialog).findByRole("button", { name: "Нажмите нужное сочетание..." });
    expect(backend.callsOf("key_capture_begin")).toHaveLength(1);
    await user.keyboard("w");
    await waitFor(() => expect(backend.callsOf("key_capture_end")).toHaveLength(1));
    expect(backend.callsOf("work_pause_toggle")).toEqual([]);

    await user.click(within(dialog).getByRole("button", { name: "Назначить" }));
    await user.click(await within(dialog).findByRole("button", { name: "Нажмите нужное сочетание..." }));
    await waitFor(() => expect(backend.callsOf("key_capture_end")).toHaveLength(2));

    await user.click(within(dialog).getByRole("button", { name: "Назначить" }));
    await within(dialog).findByRole("button", { name: "Нажмите нужное сочетание..." });
    cleanup();
    await waitFor(() => expect(backend.callsOf("key_capture_end")).toHaveLength(3));
  });
});
