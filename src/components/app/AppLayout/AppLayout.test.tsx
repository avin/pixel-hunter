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
    expect(screen.getByRole("status")).toHaveTextContent("Ожидание окна игры");
    await waitFor(() =>
      expect(backend.callsOf("engine_activate")).toEqual([
        { profileId: "diablo" },
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

  it("asks for tracker deletion after leaving its editor and keeps it on cancellation", async () => {
    const { user, backend } = renderApp(
      createFakeBackend({ profiles: [profile] }),
      ["/profiles/diablo/trackers/health"],
    );
    await user.click(
      await screen.findByRole("button", {
        name: "Удалить отслеживание «Здоровье» в навигации",
      }),
    );
    expect(
      await screen.findByRole("alertdialog", {
        name: "Удалить отслеживание «Здоровье»?",
      }),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Отмена" }));
    expect(
      screen.getByRole("heading", { level: 1, name: "Diablo" }),
    ).toBeInTheDocument();
    expect(backend.state.profiles[0].trackers).toHaveLength(1);
    expect(
      screen.getByRole("link", { name: "Открыть отслеживание «Здоровье»" }),
    ).toBeInTheDocument();
  });

  it.each([
    "Удалить отслеживание «Здоровье» в навигации",
  ])("keeps invalid input in the editor before %s", async (command) => {
    const { user, backend } = renderApp(
      createFakeBackend({ profiles: [profile] }),
      ["/profiles/diablo/trackers/health"],
    );
    const name = await screen.findByRole("textbox", { name: "Имя" });
    await user.clear(name);
    await user.click(screen.getByRole("button", { name: command }));
    expect(
      screen.getByRole("heading", { level: 1, name: "Здоровье" }),
    ).toBeInTheDocument();
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(name).toHaveFocus();
    expect(backend.callsOf("profile_delete")).toEqual([]);
    expect(backend.callsOf("tracker_delete")).toEqual([]);
    expect(
      screen.getByRole("link", { name: "Открыть отслеживание «Здоровье»" }),
    ).toHaveAttribute("aria-current", "page");
  });
});
