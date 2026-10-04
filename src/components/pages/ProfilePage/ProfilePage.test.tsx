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
  await rendered.user.click(await screen.findByRole("link", { name: profile.name }));
  return rendered;
}

async function openCreateDialog(user: UserEvent) {
  await user.click(screen.getByRole("button", { name: "Создать отслеживание" }));
  return screen.getByRole("dialog", { name: "Новое отслеживание" });
}

describe("ProfilePage", () => {
  it("shows the profile process and its trackers", async () => {
    await openProfile(profileFixture({ processName: "game.exe", trackers: [health, mana] }));

    expect(screen.getByText("game.exe")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Здоровье" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Мана" })).toBeInTheDocument();
  });

  it("offers to create a tracker in an empty profile", async () => {
    await openProfile(profileFixture());

    expect(
      screen.getByRole("heading", { name: "Отслеживаний пока нет" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Создать отслеживание" })).toBeInTheDocument();
  });

  it("creates a tracker with a name", async () => {
    const { user, backend } = await openProfile(profileFixture());
    const dialog = await openCreateDialog(user);

    await user.type(within(dialog).getByRole("textbox", { name: "Имя" }), "Здоровье");
    await user.click(within(dialog).getByRole("button", { name: "Создать" }));

    expect(await screen.findByRole("link", { name: "Здоровье" })).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(backend.state.profiles[0].trackers).toMatchObject([{ name: "Здоровье" }]);
  });

  it("does not create a tracker with an empty name", async () => {
    const { user, backend } = await openProfile(profileFixture());
    const dialog = await openCreateDialog(user);
    const nameInput = within(dialog).getByRole("textbox", { name: "Имя" });

    await user.type(nameInput, "   ");
    await user.click(within(dialog).getByRole("button", { name: "Создать" }));

    expect(within(dialog).getByText("Введите имя отслеживания")).toBeInTheDocument();
    expect(nameInput).toHaveFocus();
    expect(backend.state.profiles[0].trackers).toEqual([]);
  });

  it("deletes a tracker after confirmation", async () => {
    const { user, backend } = await openProfile(profileFixture({ trackers: [health, mana] }));

    await user.click(
      screen.getByRole("button", { name: "Удалить отслеживание «Здоровье»" }),
    );
    const dialog = screen.getByRole("alertdialog", {
      name: "Удалить отслеживание «Здоровье»?",
    });
    await user.click(within(dialog).getByRole("button", { name: "Удалить" }));

    await screen.findByRole("link", { name: "Мана" });
    expect(screen.queryByRole("link", { name: "Здоровье" })).not.toBeInTheDocument();
    expect(backend.state.profiles[0].trackers).toEqual([mana]);
  });

  it("keeps the tracker when deletion is cancelled", async () => {
    const { user, backend } = await openProfile(profileFixture({ trackers: [health] }));

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
      action: { type: "keySequence", steps: [keyStepFixture()] },
    });
    const runToggle = (name = "Здоровье") =>
      screen.getByRole("button", { name: `Запуск отслеживания «${name}»` });

    it("starts and stops a tracker", async () => {
      const { user, backend } = await openProfile(profileFixture({ trackers: [complete] }));
      expect(runToggle()).toHaveAttribute("aria-pressed", "false");
      expect(screen.getByText("Остановлено")).toBeInTheDocument();

      await user.click(runToggle());

      await waitFor(() => expect(runToggle()).toHaveAttribute("aria-pressed", "true"));
      expect(screen.getByText("Запущено")).toBeInTheDocument();
      expect(backend.state.profiles[0].trackers[0].isRunning).toBe(true);

      await user.click(runToggle());

      await waitFor(() => expect(runToggle()).toHaveAttribute("aria-pressed", "false"));
      expect(backend.state.profiles[0].trackers[0].isRunning).toBe(false);
    });

    it("cannot start a tracker without pixels", async () => {
      await openProfile(profileFixture({ trackers: [{ ...complete, pixels: [] }] }));

      expect(runToggle()).toBeDisabled();
      expect(runToggle()).toHaveAccessibleDescription("Добавьте пиксель, чтобы запустить");
    });

    it("cannot start a tracker without action steps", async () => {
      await openProfile(
        profileFixture({
          trackers: [{ ...complete, action: { type: "keySequence", steps: [] } }],
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
      action: { type: "keySequence", steps: [keyStepFixture()] },
      isRunning: true,
    });

    async function emitStatus(gameActive: boolean) {
      await act(() =>
        emit(ENGINE_STATUS_EVENT, { profileId: "profile-1", gameActive }),
      );
    }

    it("waits for the game window until it becomes active", async () => {
      await openProfile(profileFixture({ processName: "game.exe", trackers: [running] }));

      expect(screen.getByRole("status")).toHaveTextContent("Ожидание окна игры game.exe");

      await emitStatus(true);
      expect(screen.getByRole("status")).toHaveTextContent("Окно игры активно");

      await emitStatus(false);
      expect(screen.getByRole("status")).toHaveTextContent("Ожидание окна игры game.exe");
    });

    it("is hidden without running trackers", async () => {
      await openProfile(profileFixture({ trackers: [{ ...running, isRunning: false }] }));

      await emitStatus(true);

      expect(screen.queryByRole("status")).not.toBeInTheDocument();
    });
  });

  it("opens a tracker and returns to the profile", async () => {
    const { user } = await openProfile(profileFixture({ name: "Diablo", trackers: [health] }));

    await user.click(screen.getByRole("link", { name: "Здоровье" }));

    expect(screen.getByRole("heading", { level: 1, name: "Здоровье" })).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Имя" })).toHaveValue("Здоровье");

    await user.click(screen.getByRole("link", { name: "Назад" }));

    expect(screen.getByRole("heading", { level: 1, name: "Diablo" })).toBeInTheDocument();
  });
});
