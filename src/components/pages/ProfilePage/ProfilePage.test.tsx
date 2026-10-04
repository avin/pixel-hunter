import { screen, within } from "@testing-library/react";
import type { UserEvent } from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { Profile } from "@/store/types";
import {
  createFakeBackend,
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

  it("opens a tracker and returns to the profile", async () => {
    const { user } = await openProfile(profileFixture({ name: "Diablo", trackers: [health] }));

    await user.click(screen.getByRole("link", { name: "Здоровье" }));

    expect(screen.getByRole("heading", { level: 1, name: "Здоровье" })).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Имя" })).toHaveValue("Здоровье");

    await user.click(screen.getByRole("link", { name: "Назад" }));

    expect(screen.getByRole("heading", { level: 1, name: "Diablo" })).toBeInTheDocument();
  });
});
