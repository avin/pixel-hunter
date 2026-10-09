import { useEffect, useRef } from "react";
import { useBeginKeyCaptureMutation, useEndKeyCaptureMutation } from "@/store/api";

/** Keeps tracker and pause shortcuts from seeing keys while a recorder is open. */
export function useKeyCapture() {
  const [beginCapture] = useBeginKeyCaptureMutation();
  const [endCapture] = useEndKeyCaptureMutation();
  const active = useRef(false);
  const endCaptureRef = useRef(endCapture);
  endCaptureRef.current = endCapture;

  useEffect(() => {
    return () => {
      if (active.current) {
        active.current = false;
        void endCaptureRef.current();
      }
    };
  }, []);

  return {
    onPrepareRecording: async () => {
      await beginCapture().unwrap();
      active.current = true;
    },
    onRecordingChange: (isRecording: boolean) => {
      if (!isRecording && active.current) {
        active.current = false;
        void endCapture();
      }
    },
  };
}
