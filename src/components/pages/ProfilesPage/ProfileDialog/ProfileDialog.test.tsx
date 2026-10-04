import { screen, within } from "@testing-library/react";
import type { UserEvent } from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { createFakeBackend, profileFixture } from "@/test/fakeBackend";
import { renderApp } from "@/test/renderApp";

const notepad = { exeName: "notepad.exe", windowTitles: ["a.txt - Блокнот"] };
const game = { exeName: "game.exe", windowTitles: ["Diablo"] };

async function openCreateDialog(user: UserEvent) {
  await user.click(await screen.findByRole("button", { name: "Создать профиль" }));
  return screen.getByRole("dialog", { name: "Новый профиль" });
}

async function chooseProcess(user: UserEvent, exeName: string) {
  await user.click(screen.getByRole("combobox", { name: "Процесс игры" }));
  await user.click(await screen.findByRole("option", { name: new RegExp(exeName) }));
}

const nameInput = () => screen.getByRole("textbox", { name: "Имя" });
const nameError = () => screen.queryByText("Введите имя профиля");
const processError = () => screen.queryByText("Выберите процесс игры");

describe("ProfileDialog", () => {
  it("creates a profile for a process with a window", async () => {
    const { user, backend } = renderApp(createFakeBackend({ processes: [notepad, game] }));
    await openCreateDialog(user);

    await user.type(nameInput(), "Diablo");
    await user.click(screen.getByRole("combobox", { name: "Процесс игры" }));
    const option = await screen.findByRole("option", { name: /game\.exe/ });
    expect(option).toHaveTextContent("Diablo");
    await user.click(option);
    await user.click(screen.getByRole("button", { name: "Создать" }));

    const row = (await screen.findByRole("link", { name: "Diablo" })).closest("tr")!;
    expect(within(row).getByText("game.exe")).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(backend.state.profiles).toMatchObject([
      { name: "Diablo", processName: "game.exe" },
    ]);
  });

  it("shows errors of all empty fields on submit and focuses the first one", async () => {
    const { user, backend } = renderApp(createFakeBackend({ processes: [game] }));
    await openCreateDialog(user);

    await user.click(screen.getByRole("button", { name: "Создать" }));

    expect(nameError()).toBeInTheDocument();
    expect(processError()).toBeInTheDocument();
    expect(nameInput()).toHaveFocus();
    expect(nameInput()).toHaveAccessibleDescription("Введите имя профиля");
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(backend.state.profiles).toEqual([]);
  });

  it("shows the name error after the edited field loses focus", async () => {
    const { user } = renderApp();
    await openCreateDialog(user);

    await user.click(nameInput());
    await user.tab();
    expect(nameError()).not.toBeInTheDocument();

    await user.type(nameInput(), "D");
    await user.clear(nameInput());
    expect(nameError()).not.toBeInTheDocument();

    await user.tab();
    expect(nameError()).toBeInTheDocument();
  });

  it("updates a shown error live while typing", async () => {
    const { user } = renderApp();
    await openCreateDialog(user);
    await user.click(screen.getByRole("button", { name: "Создать" }));

    await user.type(nameInput(), "D");
    expect(nameError()).not.toBeInTheDocument();

    await user.clear(nameInput());
    expect(nameError()).toBeInTheDocument();
  });

  it("hides the process error once a process is chosen", async () => {
    const { user } = renderApp(createFakeBackend({ processes: [game] }));
    await openCreateDialog(user);
    await user.click(screen.getByRole("button", { name: "Создать" }));

    await chooseProcess(user, "game.exe");

    expect(processError()).not.toBeInTheDocument();
  });

  it("starts without errors when opened again", async () => {
    const { user } = renderApp();
    await openCreateDialog(user);
    await user.click(screen.getByRole("button", { name: "Создать" }));
    await user.click(screen.getByRole("button", { name: "Отмена" }));

    await openCreateDialog(user);

    expect(nameError()).not.toBeInTheDocument();
    expect(processError()).not.toBeInTheDocument();
  });

  it("refreshes the process list", async () => {
    const { user, backend } = renderApp(createFakeBackend({ processes: [notepad] }));
    await openCreateDialog(user);
    await user.click(screen.getByRole("combobox", { name: "Процесс игры" }));
    expect(await screen.findByRole("option", { name: /notepad\.exe/ })).toBeInTheDocument();
    expect(screen.queryByRole("option", { name: /game\.exe/ })).not.toBeInTheDocument();
    await user.keyboard("{Escape}");

    backend.state.processes.push(game);
    await user.click(screen.getByRole("button", { name: "Обновить" }));
    await user.click(screen.getByRole("combobox", { name: "Процесс игры" }));

    expect(await screen.findByRole("option", { name: /game\.exe/ })).toBeInTheDocument();
  });

  it("changes the process of an existing profile", async () => {
    const diablo = profileFixture({ name: "Diablo", processName: "game.exe" });
    const { user, backend } = renderApp(
      createFakeBackend({ profiles: [diablo], processes: [notepad] }),
    );

    await user.click(
      await screen.findByRole("button", { name: "Изменить профиль «Diablo»" }),
    );
    expect(screen.getByRole("dialog", { name: "Редактирование профиля" })).toBeInTheDocument();
    expect(nameInput()).toHaveValue("Diablo");
    expect(screen.getByRole("combobox", { name: "Процесс игры" })).toHaveTextContent("game.exe");

    await chooseProcess(user, "notepad.exe");
    await user.click(screen.getByRole("button", { name: "Сохранить" }));

    const row = (await screen.findByRole("link", { name: "Diablo" })).closest("tr")!;
    expect(await within(row).findByText("notepad.exe")).toBeInTheDocument();
    expect(backend.state.profiles[0]).toMatchObject({ processName: "notepad.exe" });
  });

  it("shows a command error and keeps the dialog open", async () => {
    const backend = createFakeBackend({ processes: [game] });
    backend.failOn("profile_create", "Не удалось сохранить конфигурацию: диск заполнен");
    const { user } = renderApp(backend);
    await openCreateDialog(user);

    await user.type(nameInput(), "Diablo");
    await chooseProcess(user, "game.exe");
    await user.click(screen.getByRole("button", { name: "Создать" }));

    expect(
      await screen.findByText("Не удалось сохранить конфигурацию: диск заполнен"),
    ).toBeInTheDocument();
    expect(screen.getByRole("dialog", { name: "Новый профиль" })).toBeInTheDocument();
  });
});
