import { describe, expect, it } from "vitest";
import { toPhysicalPoint, toPhysicalRect } from "./physicalPoint";

const screen = { width: 3840, height: 2160 };

describe("toPhysicalPoint", () => {
  it.each([1, 1.25, 1.5])(
    "maps every physical pixel back to itself at dpr %s",
    (dpr) => {
      for (let physical = 0; physical < 400; physical++) {
        const client = physical / dpr;
        expect(toPhysicalPoint(client, client, dpr, screen)).toEqual({
          x: physical,
          y: physical,
        });
      }
    },
  );

  it("picks the pixel under a fractional cursor position", () => {
    expect(toPhysicalPoint(541.6, 693.9, 1.5, screen)).toEqual({ x: 812, y: 1040 });
  });

  it("clamps points past the edges of the snapshot", () => {
    expect(toPhysicalPoint(-1, -0.5, 1.25, screen)).toEqual({ x: 0, y: 0 });
    expect(toPhysicalPoint(3072, 1728, 1.25, screen)).toEqual({ x: 3839, y: 2159 });
    expect(toPhysicalPoint(5000, 5000, 1, screen)).toEqual({ x: 3839, y: 2159 });
  });
});

describe("toPhysicalRect", () => {
  it.each([1, 1.25, 1.5])("normalizes a reverse drag at dpr %s", (dpr) => {
    expect(toPhysicalRect({ x: 120 / dpr, y: 90 / dpr }, { x: 20 / dpr, y: 30 / dpr }, dpr, screen))
      .toEqual({ x: 20, y: 30, width: 100, height: 60 });
  });

  it.each([1, 1.25, 1.5])("includes the final row and column at dpr %s", (dpr) => {
    expect(toPhysicalRect(
      { x: 3830 / dpr, y: 2150 / dpr },
      { x: 3840 / dpr, y: 2160 / dpr }, dpr, screen,
    )).toEqual({ x: 3830, y: 2150, width: 10, height: 10 });
  });

  it("clamps rectangle boundaries without making an extra pixel", () => {
    expect(toPhysicalRect({ x: -10, y: -20 }, { x: 5000, y: 5000 }, 1, screen))
      .toEqual({ x: 0, y: 0, width: 3840, height: 2160 });
  });

  it("keeps a click empty so the backend can reject it", () => {
    expect(toPhysicalRect({ x: 5, y: 6 }, { x: 5, y: 6 }, 1.5, screen))
      .toEqual({ x: 7, y: 9, width: 0, height: 0 });
  });
});
