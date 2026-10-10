export interface Size {
  width: number;
  height: number;
}

export interface Point {
  x: number;
  y: number;
}

/** Rectangle edges are half-open: the far edge may equal the full image size. */
export function toPhysicalRect(start: Point, end: Point, devicePixelRatio: number, size: Size) {
  const edge = (value: number, length: number) => Math.min(Math.max(Math.floor(value * devicePixelRatio + EPSILON), 0), length);
  const left = edge(Math.min(start.x, end.x), size.width);
  const top = edge(Math.min(start.y, end.y), size.height);
  const right = edge(Math.max(start.x, end.x), size.width);
  const bottom = edge(Math.max(start.y, end.y), size.height);
  return { x: left, y: top, width: right - left, height: bottom - top };
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
