import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { createFakeBackend, profileFixture } from "@/test/fakeBackend";
import { renderApp } from "@/test/renderApp";

describe("ActionSection", () => {
  it("gives a new tracker an empty key sequence", async () => {
    const { user } = renderApp(createFakeBackend({ profiles: [profileFixture()] }));
    await user.click(await screen.findByRole("link", { name: "Diablo" }));
    await user.click(screen.getByRole("button", { name: "Создать отслеживание" }));
    await user.type(screen.getByRole("textbox", { name: "Имя" }), "Здоровье");
    await user.click(screen.getByRole("button", { name: "Создать" }));
    await user.click(await screen.findByRole("link", { name: "Здоровье" }));

    const typeSelect = screen.getByRole("combobox", { name: "Тип" });
    expect(typeSelect).toHaveTextContent("Нажатие клавиш");
    expect(await screen.findByText("Нажатия не настроены")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Добавить нажатие" })).toBeEnabled();

    await user.click(typeSelect);
    expect(
      await screen.findByRole("option", { name: "Нажатие клавиш" }),
    ).toBeInTheDocument();
  });
});
