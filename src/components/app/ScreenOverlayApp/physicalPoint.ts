export interface Size {
  width: number;
  height: number;
}

export interface Point {
  x: number;
  y: number;
}

/**
 * The cursor sits on whole physical pixels, so `client * dpr` lands on an integer
 * up to floating-point error; the epsilon keeps `floor` from dropping to the previous pixel.
 */
const EPSILON = 1e-4;

export function toPhysicalPoint(
  clientX: number,
  clientY: number,
  devicePixelRatio: number,
  size: Size,
): Point {
  return {
    x: toPhysical(clientX, devicePixelRatio, size.width),
    y: toPhysical(clientY, devicePixelRatio, size.height),
  };
}

function toPhysical(client: number, devicePixelRatio: number, length: number) {
  const physical = Math.floor(client * devicePixelRatio + EPSILON);
  return Math.min(Math.max(physical, 0), length - 1);
}
