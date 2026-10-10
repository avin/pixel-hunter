import { useLayoutEffect, useRef, useState } from "react";
import type { Point } from "../physicalPoint";

interface Props { cursor: Point; text: string }

export function CursorHint({ cursor, text }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  useLayoutEffect(() => {
    if (ref.current) setSize({ width: ref.current.offsetWidth, height: ref.current.offsetHeight });
  }, [text]);
  const left = Math.max(0, Math.min(cursor.x + 16, window.innerWidth - size.width));
  const top = Math.max(0, Math.min(cursor.y + 16, window.innerHeight - size.height));
  return <div ref={ref} role="status" className="pointer-events-none fixed z-10 max-w-full bg-gray-200 px-2 py-1 text-sm text-black" style={{ left, top }}>{text}</div>;
}
