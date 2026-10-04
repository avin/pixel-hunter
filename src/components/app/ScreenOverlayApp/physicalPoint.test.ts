import { describe, expect, it } from "vitest";
import { toPhysicalPoint } from "./physicalPoint";

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
