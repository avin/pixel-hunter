import { screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { createFakeBackend, profileFixture } from "@/test/fakeBackend";
import { renderApp } from "@/test/renderApp";

const diablo = profileFixture({ id: "diablo", name: "Diablo", processName: "game.exe" });
const poe = profileFixture({ id: "poe", name: "Path of Exile", processName: "PathOfExile.exe" });

describe("ProfilesPage", () => {
  it("lists saved profiles with their process", async () => {
    renderApp(createFakeBackend({ profiles: [diablo, poe] }));

    const diabloRow = (await screen.findByRole("link", { name: "Diablo" })).closest("tr")!;
    expect(within(diabloRow).getByText("game.exe")).toBeInTheDocument();
    const poeRow = screen.getByRole("link", { name: "Path of Exile" }).closest("tr")!;
    expect(within(poeRow).getByText("PathOfExile.exe")).toBeInTheDocument();
  });

  it("offers to create a profile when there are none", async () => {
    renderApp();

    expect(
      await screen.findByRole("heading", { name: "Профилей пока нет" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Создать профиль" })).toBeInTheDocument();
  });

  it("reports a failed load and retries it", async () => {
    const backend = createFakeBackend({ profiles: [diablo] });
    backend.failOn("profiles_list", "Не удалось прочитать конфигурацию");
    const { user } = renderApp(backend);

    expect(await screen.findByRole("alert")).toHaveTextContent("Не удалось загрузить профили.");
    expect(screen.getByText("Не удалось прочитать конфигурацию")).toBeInTheDocument();

    backend.failOn("profiles_list", undefined);
    await user.click(screen.getByRole("button", { name: "Повторить" }));

    expect(await screen.findByRole("link", { name: "Diablo" })).toBeInTheDocument();
  });

  it("deletes a profile after confirmation", async () => {
    const { user, backend } = renderApp(createFakeBackend({ profiles: [diablo, poe] }));

    await user.click(
      await screen.findByRole("button", { name: "Удалить профиль «Diablo»" }),
    );
    const dialog = screen.getByRole("alertdialog", { name: "Удалить профиль «Diablo»?" });
    await user.click(within(dialog).getByRole("button", { name: "Удалить" }));

    expect(await screen.findByRole("link", { name: "Path of Exile" })).toBeInTheDocument();
    expect(screen.queryByRole("link", { name: "Diablo" })).not.toBeInTheDocument();
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(backend.state.profiles.map(({ name }) => name)).toEqual(["Path of Exile"]);
  });

  it("keeps the profile when deletion is cancelled", async () => {
    const { user, backend } = renderApp(createFakeBackend({ profiles: [diablo] }));

    await user.click(
      await screen.findByRole("button", { name: "Удалить профиль «Diablo»" }),
    );
    await user.click(screen.getByRole("button", { name: "Отмена" }));

    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Diablo" })).toBeInTheDocument();
    expect(backend.state.profiles).toEqual([diablo]);
  });

  it("opens a profile and returns to the list", async () => {
    const { user } = renderApp(createFakeBackend({ profiles: [diablo] }));

    await user.click(await screen.findByRole("link", { name: "Diablo" }));

    expect(screen.getByRole("heading", { level: 1, name: "Diablo" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Отслеживаний пока нет" })).toBeInTheDocument();

    await user.click(screen.getByRole("link", { name: "Назад" }));

    expect(screen.getByRole("heading", { level: 1, name: "Профили" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Diablo" })).toBeInTheDocument();
  });
});
