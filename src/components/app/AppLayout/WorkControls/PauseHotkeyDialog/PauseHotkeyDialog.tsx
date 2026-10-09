import { useEffect, useRef, useState } from "react";
import { useKeyCapture } from "@/hooks/useKeyCapture";
import { comboLabel } from "@/components/pages/TrackerPage/ActionSection/KeySequenceEditor/comboLabel";
import { KeyRecorder } from "@/components/pages/TrackerPage/ActionSection/KeySequenceEditor/KeyRecorder/KeyRecorder";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Skeleton } from "@/components/ui/skeleton";
import {
  useBeginPauseHotkeyMutation,
  useCancelPauseHotkeyMutation,
  useClearPauseHotkeyMutation,
  useGetSettingsQuery,
  useKeyCodesListQuery,
  useSetPauseHotkeyMutation,
} from "@/store/api";
import type { KeyCombo } from "@/store/types";

interface Props {
  onClose: () => void;
}

export function PauseHotkeyDialog({ onClose }: Props) {
  const settings = useGetSettingsQuery();
  const keyCodes = useKeyCodesListQuery();
  const [setPauseHotkey] = useSetPauseHotkeyMutation();
  const [clearPauseHotkey] = useClearPauseHotkeyMutation();
  const [beginHotkey] = useBeginPauseHotkeyMutation();
  const [cancelHotkey] = useCancelPauseHotkeyMutation();
  const [isRejected, setIsRejected] = useState(false);
  const capture = useKeyCapture();
  const began = useRef(false);
  const cancelHotkeyRef = useRef(cancelHotkey);
  cancelHotkeyRef.current = cancelHotkey;

  const hotkey = settings.data?.pauseHotkey ?? null;
  const labels = new Map((keyCodes.data ?? []).map(({ code, label }) => [code, label]));
  const isReady = Boolean(settings.data && keyCodes.data);
  const hasFailed = settings.isError || keyCodes.isError;

  useEffect(() => {
    return () => {
      if (began.current) void cancelHotkeyRef.current();
    };
  }, []);

  async function onPrepareRecording() {
    setIsRejected(false);
    await beginHotkey().unwrap();
    try {
      await capture.onPrepareRecording();
    } catch (error) {
      began.current = false;
      void cancelHotkey();
      throw error;
    }
    began.current = true;
  }

  function onRecordingChange(isRecording: boolean) {
    capture.onRecordingChange(isRecording);
    if (!isRecording && began.current) {
      began.current = false;
      void cancelHotkey();
    }
  }

  function onRecord(combo: KeyCombo) {
    began.current = false;
    setIsRejected(false);
    void setPauseHotkey(combo);
  }

  return (
    <Dialog open onOpenChange={(isOpen) => !isOpen && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Сочетание для паузы</DialogTitle>
          {isReady && (
            <DialogDescription>
              {hotkey ? (
                <>
                  Сейчас назначено{" "}
                  <span className="font-mono text-foreground">{comboLabel(hotkey, labels)}</span>.
                </>
              ) : (
                "Сочетание не назначено."
              )}
            </DialogDescription>
          )}
        </DialogHeader>
        {hasFailed ? (
          <p role="alert">Не удалось загрузить сочетание</p>
        ) : isReady ? (
          <KeyRecorder
            variant="outline"
            supportedCodes={new Set(labels.keys())}
            onRecord={onRecord}
            onUnsupported={() => setIsRejected(true)}
            onPrepareRecording={onPrepareRecording}
            onRecordingChange={onRecordingChange}
            recordingLabel="Нажмите нужное сочетание..."
          >
            Назначить
          </KeyRecorder>
        ) : (
          <Skeleton className="h-8 w-28" />
        )}
        {isRejected && (
          <p role="alert" className="text-destructive">
            Эта клавиша не поддерживается
          </p>
        )}
        {hotkey && (
          <DialogFooter>
            <Button variant="outline" onClick={() => void clearPauseHotkey()}>
              Сбросить сочетание
            </Button>
          </DialogFooter>
        )}
      </DialogContent>
    </Dialog>
  );
}
