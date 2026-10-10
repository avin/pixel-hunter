import { useEffect, useEffectEvent, useLayoutEffect, useRef, useState } from "react";
import { usePickerPixelsMutation } from "@/store/api";
import type { PickerFrame } from "@/store/types";
import type { Size } from "../../physicalPoint";

interface Props {
  frame: PickerFrame;
  onLoad: (size: Size) => void;
  onReady: () => void;
  onError: () => void;
  children?: (size: Size) => React.ReactNode;
}

/** The physical RGBA frame is drawn directly, before acknowledging a visible picker. */
export function PickerCanvas({ frame, onLoad, onReady, onError, children }: Props) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const [size, setSize] = useState<Size>();
  const [getPixels, { reset }] = usePickerPixelsMutation();
  const loaded = useEffectEvent(onLoad);
  const ready = useEffectEvent(onReady);
  const failed = useEffectEvent(onError);
  const resetPixels = useEffectEvent(reset);

  useEffect(() => {
    let isActive = true;
    void getPixels(frame).unwrap().then((buffer) => {
      if (!isActive) { resetPixels(); return; }
      if (buffer.byteLength < 8) throw new Error("Неполный кадр");
      const header = new DataView(buffer);
      const width = header.getUint32(0, true);
      const height = header.getUint32(4, true);
      if (!width || !height || buffer.byteLength !== 8 + width * height * 4) {
        throw new Error("Неверный размер кадра");
      }
      const target = canvas.current;
      const context = target?.getContext("2d");
      if (!target || !context) throw new Error("Не удалось подготовить кадр");
      target.width = width;
      target.height = height;
      context.putImageData(new ImageData(new Uint8ClampedArray(buffer, 8), width, height), 0, 0);
      const nextSize = { width, height };
      setSize(nextSize);
      loaded(nextSize);
      resetPixels();
    }).catch(() => {
      resetPixels();
      if (isActive) failed();
    });
    return () => {
      isActive = false;
      resetPixels();
    };
  }, [frame, getPixels]);

  // Hidden WebViews suspend animation frames. putImageData above has completed;
  // the layout effect confirms that the canvas dimensions are also committed.
  useLayoutEffect(() => {
    if (size) ready();
  }, [size]);

  const style = size ? { width: size.width / window.devicePixelRatio, height: size.height / window.devicePixelRatio } : undefined;
  return (
    <div className="relative shrink-0" style={style}>
      <canvas ref={canvas} role="img" aria-label="Снимок экрана" style={style} className={size ? "absolute top-0 left-0 max-w-none" : "invisible"} />
      {size && children?.(size)}
    </div>
  );
}
