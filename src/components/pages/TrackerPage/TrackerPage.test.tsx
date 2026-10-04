import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { Tracker } from "@/store/types";
import {
  createFakeBackend,
  profileFixture,
  trackerFixture,
} from "@/test/fakeBackend";
import { renderApp } from "@/test/renderApp";

async function openTracker(tracker: Tracker = trackerFixture()) {
  const rendered = renderApp(
    createFakeBackend({ profiles: [profileFixture({ trackers: [tracker] })] }),
  );
  await rendered.user.click(await screen.findByRole("link", { name: "Diablo" }));
  await rendered.user.click(screen.getByRole("link", { name: tracker.name }));
  return rendered;
}

const nameInput = () => screen.getByRole("textbox", { name: "Имя" });
const minInput = () => screen.getByRole("spinbutton", { name: "Минимум" });
const maxInput = () => screen.getByRole("spinbutton", { name: "Максимум" });
const pauseInput = () =>
  screen.getByRole("spinbutton", { name: "Пауза до возобновления опроса, мс" });
const saveButton = () => screen.getByRole("button", { name: "Сохранить" });

describe("TrackerPage", () => {
  it("shows default timings of a new tracker", async () => {
    const { user } = renderApp(createFakeBackend({ profiles: [profileFixture()] }));
    await user.click(await screen.findByRole("link", { name: "Diablo" }));
    await user.click(screen.getByRole("button", { name: "Создать отслеживание" }));
    await user.type(screen.getByRole("textbox", { name: "Имя" }), "Здоровье");
    await user.click(screen.getByRole("button", { name: "Создать" }));
    await user.click(await screen.findByRole("link", { name: "Здоровье" }));

    expect(minInput()).toHaveValue(100);
    expect(maxInput()).toHaveValue(300);
    expect(pauseInput()).toHaveValue(1000);
  });

  it("saves a renamed tracker with new timings", async () => {
    const { user, backend } = await openTracker();

    await user.clear(nameInput());
    await user.type(nameInput(), "Мана");
    await user.clear(minInput());
    await user.type(minInput(), "50");
    await user.clear(maxInput());
    await user.type(maxInput(), "80");
    await user.click(saveButton());

    expect(await screen.findByText("Отслеживание сохранено")).toBeInTheDocument();
    expect(
      await screen.findByRole("heading", { level: 1, name: "Мана" }),
    ).toBeInTheDocument();
    expect(backend.state.profiles[0].trackers[0]).toMatchObject({
      name: "Мана",
      preActionDelayMs: { min: 50, max: 80 },
      resumeDelayMs: 1000,
    });
  });

  it("saves a zero pause", async () => {
    const { user, backend } = await openTracker();

    await user.clear(pauseInput());
    await user.type(pauseInput(), "0");
    await user.click(saveButton());

    expect(await screen.findByText("Отслеживание сохранено")).toBeInTheDocument();
    expect(backend.state.profiles[0].trackers[0].resumeDelayMs).toBe(0);
  });

  it("rejects a delay range with minimum above maximum", async () => {
    const tracker = trackerFixture();
    const { user, backend } = await openTracker(tracker);

    await user.clear(minInput());
    await user.type(minInput(), "500");
    await user.clear(maxInput());
    await user.type(maxInput(), "200");
    await user.click(saveButton());

    const error = "Минимум не может быть больше максимума";
    expect(screen.getByText(error)).toBeInTheDocument();
    expect(minInput()).toHaveAccessibleDescription(error);
    expect(maxInput()).toHaveAccessibleDescription(error);
    expect(minInput()).toHaveFocus();
    expect(backend.state.profiles[0].trackers[0]).toEqual(tracker);
  });

  it("shows the range error only after focus leaves the range", async () => {
    const { user } = await openTracker();

    await user.clear(minInput());
    await user.type(minInput(), "500");
    await user.click(maxInput());
    expect(screen.queryByText("Минимум не может быть больше максимума")).not.toBeInTheDocument();

    await user.click(pauseInput());
    expect(screen.getByText("Минимум не может быть больше максимума")).toBeInTheDocument();

    await user.clear(maxInput());
    await user.type(maxInput(), "600");
    expect(screen.queryByText("Минимум не может быть больше максимума")).not.toBeInTheDocument();
  });

  it("rejects a negative pause", async () => {
    const tracker = trackerFixture();
    const { user, backend } = await openTracker(tracker);

    await user.clear(pauseInput());
    await user.type(pauseInput(), "-100");
    await user.tab();

    expect(pauseInput()).toHaveAccessibleDescription(
      expect.stringContaining("Введите целое неотрицательное число"),
    );

    await user.click(saveButton());

    expect(pauseInput()).toHaveFocus();
    expect(screen.queryByText("Отслеживание сохранено")).not.toBeInTheDocument();
    expect(backend.state.profiles[0].trackers[0]).toEqual(tracker);
  });

  it("rejects an empty name", async () => {
    const tracker = trackerFixture();
    const { user, backend } = await openTracker(tracker);

    await user.clear(nameInput());
    await user.click(saveButton());

    expect(screen.getByText("Введите имя отслеживания")).toBeInTheDocument();
    expect(nameInput()).toHaveFocus();
    expect(backend.state.profiles[0].trackers[0]).toEqual(tracker);
  });
});
