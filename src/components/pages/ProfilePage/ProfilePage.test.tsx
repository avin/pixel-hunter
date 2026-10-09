import { emit } from "@tauri-apps/api/event";
import { act, screen, waitFor, within } from "@testing-library/react";
import type { UserEvent } from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { ENGINE_STATUS_EVENT } from "@/hooks/useEngineEvents";
import type { Profile } from "@/store/types";
import {
  createFakeBackend,
  keyStepFixture,
  pixelFixture,
  profileFixture,
  trackerFixture,
} from "@/test/fakeBackend";
import { renderApp } from "@/test/renderApp";

const health = trackerFixture({ id: "health", name: "Здоровье" });
const mana = trackerFixture({ id: "mana", name: "Мана" });

async function openProfile(profile: Profile) {
  const rendered = renderApp(createFakeBackend({ profiles: [profile] }));
  await rendered.user.click(
    await screen.findByRole("link", { name: profile.name }),
  );
  return rendered;
}

async function openCreateDialog(user: UserEvent) {
  await user.click(
    screen.getByRole("button", { name: "Создать отслеживание" }),
  );
  return screen.getByRole("dialog", { name: "Новое отслеживание" });
}

describe("ProfilePage", () => {
  it("offers profile editing and confirmed deletion in the profile view", async () => {
    const { user, backend } = await openProfile(profileFixture());
    await user.click(screen.getByRole("button", { name: "Изменить профиль «Diablo»" }));
    expect(await screen.findByRole("dialog")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Отмена" }));
    await user.click(screen.getByRole("button", { name: "Удалить профиль «Diablo»" }));
    await screen.findByRole("alertdialog", { name: "Удалить профиль «Diablo»?" });
    await user.click(screen.getByRole("button", { name: "Отмена" }));
    expect(backend.state.profiles).toHaveLength(1);
    expect(screen.getByRole("heading", { name: "Diablo" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Удалить профиль «Diablo»" }));
    await user.click(screen.getByRole("button", { name: "Удалить" }));
    await screen.findByRole("heading", { name: "Профили" });
    expect(backend.state.profiles).toHaveLength(0);
    await waitFor(() => expect(backend.callsOf("engine_deactivate")).toHaveLength(1));
  });

  it("keeps the profile open when deletion fails", async () => {
    const { user, backend } = await openProfile(profileFixture());
    backend.failOn("profile_delete", "Не удалось удалить профиль");
    await user.click(screen.getByRole("button", { name: "Удалить профиль «Diablo»" }));
    await user.click(screen.getByRole("button", { name: "Удалить" }));
    await screen.findByText("Не удалось удалить профиль");
    expect(screen.getByRole("alertdialog", { name: "Удалить профиль «Diablo»?" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Отмена" }));
    expect(screen.getByRole("heading", { name: "Diablo" })).toBeInTheDocument();
    expect(backend.state.profiles).toHaveLength(1);
    expect(backend.callsOf("engine_deactivate")).toHaveLength(0);
  });

  it("shows the profile process and its trackers", async () => {
    await openProfile(
      profileFixture({ processName: "game.exe", trackers: [health, mana] }),
    );

    expect(screen.getByText("game.exe")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Здоровье" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Мана" })).toBeInTheDocument();
  });

  it("offers to create a tracker in an empty profile", async () => {
    await openProfile(profileFixture());

    expect(
      screen.getByRole("heading", { name: "Отслеживаний пока нет" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Создать отслеживание" }),
    ).toBeInTheDocument();
  });

  it("creates a tracker with a name", async () => {
    const { user, backend } = await openProfile(profileFixture());
    const dialog = await openCreateDialog(user);

    await user.type(
      within(dialog).getByRole("textbox", { name: "Имя" }),
      "Здоровье",
    );
    await user.click(within(dialog).getByRole("button", { name: "Создать" }));

    expect(
      await screen.findByRole("link", { name: "Здоровье" }),
    ).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(backend.state.profiles[0].trackers).toMatchObject([
      { name: "Здоровье" },
    ]);
  });

  it("does not create a tracker with an empty name", async () => {
    const { user, backend } = await openProfile(profileFixture());
    const dialog = await openCreateDialog(user);
    const nameInput = within(dialog).getByRole("textbox", { name: "Имя" });

    await user.type(nameInput, "   ");
    await user.click(within(dialog).getByRole("button", { name: "Создать" }));

    expect(
      within(dialog).getByText("Введите имя отслеживания"),
    ).toBeInTheDocument();
    expect(nameInput).toHaveFocus();
    expect(backend.state.profiles[0].trackers).toEqual([]);
  });

  it("deletes a tracker after confirmation", async () => {
    const { user, backend } = await openProfile(
      profileFixture({ trackers: [health, mana] }),
    );

    await user.click(
      screen.getByRole("button", { name: "Удалить отслеживание «Здоровье»" }),
    );
    const dialog = screen.getByRole("alertdialog", {
      name: "Удалить отслеживание «Здоровье»?",
    });
    await user.click(within(dialog).getByRole("button", { name: "Удалить" }));

    await screen.findByRole("link", { name: "Мана" });
    expect(
      screen.queryByRole("link", { name: "Здоровье" }),
    ).not.toBeInTheDocument();
    expect(backend.state.profiles[0].trackers).toEqual([mana]);
  });

  it("keeps the tracker when deletion is cancelled", async () => {
    const { user, backend } = await openProfile(
      profileFixture({ trackers: [health] }),
    );

    await user.click(
      screen.getByRole("button", { name: "Удалить отслеживание «Здоровье»" }),
    );
    await user.click(screen.getByRole("button", { name: "Отмена" }));

    expect(screen.getByRole("link", { name: "Здоровье" })).toBeInTheDocument();
    expect(backend.state.profiles[0].trackers).toEqual([health]);
  });

  describe("run toggle", () => {
    const complete = trackerFixture({
      id: "health",
      name: "Здоровье",
      pixels: [pixelFixture()],
      action: { type: "keySequence", sound: { id: null, volumePercent: 50 }, steps: [keyStepFixture()] },
    });
    const runToggle = (name = "Здоровье") =>
      screen.getByRole("button", { name: new RegExp(`^(Старт|Стоп) отслеживания «${name}»$`) });

    it("starts and stops a tracker", async () => {
      const { user, backend } = await openProfile(
        profileFixture({ trackers: [complete] }),
      );
      expect(runToggle()).toHaveAttribute("aria-pressed", "false");
      expect(runToggle()).toHaveTextContent("Старт");
      expect(screen.queryByText("Остановлено")).not.toBeInTheDocument();

      await user.click(runToggle());

      await waitFor(() =>
        expect(runToggle()).toHaveAttribute("aria-pressed", "true"),
      );
      expect(runToggle()).toHaveTextContent("Стоп");
      expect(screen.queryByText("Запущено")).not.toBeInTheDocument();
      expect(backend.state.profiles[0].trackers[0].isRunning).toBe(true);

      await user.click(runToggle());

      await waitFor(() =>
        expect(runToggle()).toHaveAttribute("aria-pressed", "false"),
      );
      expect(backend.state.profiles[0].trackers[0].isRunning).toBe(false);
      expect(runToggle()).toHaveTextContent("Старт");
    });

    it("cannot start a tracker without pixels", async () => {
      await openProfile(
        profileFixture({ trackers: [{ ...complete, pixels: [] }] }),
      );

      expect(runToggle()).toBeDisabled();
      expect(runToggle()).toHaveAccessibleDescription(
        "Добавьте пиксель, чтобы запустить",
      );
    });

    it("starts a network disconnect tracker that has a pixel", async () => {
      const { user, backend } = await openProfile(
        profileFixture({
          trackers: [{
            ...complete,
            action: { type: "networkDisconnect", sound: { id: null, volumePercent: 50 } },
          }],
        }),
      );

      expect(runToggle()).toBeEnabled();
      await user.click(runToggle());

      await waitFor(() => expect(runToggle()).toHaveAttribute("aria-pressed", "true"));
      expect(backend.state.profiles[0].trackers[0].isRunning).toBe(true);
    });

    it("cannot start a network disconnect tracker without pixels", async () => {
      await openProfile(
        profileFixture({
          trackers: [{
            ...complete,
            pixels: [],
            action: { type: "networkDisconnect", sound: { id: null, volumePercent: 50 } },
          }],
        }),
      );

      expect(runToggle()).toBeDisabled();
      expect(runToggle()).toHaveAccessibleDescription("Добавьте пиксель, чтобы запустить");
    });

    it("cannot start a tracker without action steps", async () => {
      await openProfile(
        profileFixture({
          trackers: [
            { ...complete, action: { type: "keySequence", sound: { id: null, volumePercent: 50 }, steps: [] } },
          ],
        }),
      );

      expect(runToggle()).toBeDisabled();
      expect(runToggle()).toHaveAccessibleDescription(
        "Добавьте шаг действия, чтобы запустить",
      );
    });

    it("shows the saved run state when the profile is opened again", async () => {
      const { user } = await openProfile(
        profileFixture({
          trackers: [
            { ...complete, isRunning: true },
            { ...complete, id: "mana", name: "Мана" },
          ],
        }),
      );

      await user.click(screen.getByRole("link", { name: "Назад" }));
      await user.click(await screen.findByRole("link", { name: "Diablo" }));

      expect(runToggle("Здоровье")).toHaveAttribute("aria-pressed", "true");
      expect(runToggle("Мана")).toHaveAttribute("aria-pressed", "false");
    });
  });

  describe("game window indicator", () => {
    const running = trackerFixture({
      pixels: [pixelFixture()],
      action: { type: "keySequence", sound: { id: null, volumePercent: 50 }, steps: [keyStepFixture()] },
      isRunning: true,
    });

    async function emitStatus(gameActive: boolean) {
      await act(() =>
        emit(ENGINE_STATUS_EVENT, { profileId: "profile-1", gameActive }),
      );
    }

    it("waits for the game window until it becomes active", async () => {
      await openProfile(
        profileFixture({ processName: "game.exe", trackers: [running] }),
      );

      expect(screen.getByText("Ожидание окна игры game.exe")).toBeInTheDocument();

      await emitStatus(true);
      expect(screen.getByText("Окно игры активно")).toBeInTheDocument();

      await emitStatus(false);
      expect(screen.getByText("Ожидание окна игры game.exe")).toBeInTheDocument();
    });

    it("shows window activity separately from the saved run count", async () => {
      await openProfile(
        profileFixture({ trackers: [{ ...running, isRunning: false }] }),
      );

      await emitStatus(true);

      expect(screen.getByText("Окно игры активно")).toBeInTheDocument();
      expect(screen.getByText("Запущено: 0/1")).toBeInTheDocument();
    });
  });

  it("opens a tracker and returns to the profile", async () => {
    const { user } = await openProfile(
      profileFixture({ name: "Diablo", trackers: [health] }),
    );

    await user.click(screen.getByRole("link", { name: "Здоровье" }));

    expect(
      screen.getByRole("heading", { level: 1, name: "Здоровье" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Имя" })).toHaveValue(
      "Здоровье",
    );

    await user.click(screen.getByRole("link", { name: "Назад" }));

    expect(
      screen.getByRole("heading", { level: 1, name: "Diablo" }),
    ).toBeInTheDocument();
  });
});
