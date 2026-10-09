import { Popover } from "@base-ui/react/popover";
import { ChevronDown, Play } from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";
import { cn } from "cn";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Skeleton } from "@/components/ui/skeleton";
import { useActionSoundsListQuery, useActionSoundPreviewMutation, useActionSoundStopPreviewMutation } from "@/store/api";
import type { SoundSettings as Settings } from "@/store/types";

interface Props {
  sound: Settings;
  onChange: (sound: Settings) => void;
}

export function SoundSettings({ sound, onChange }: Props) {
  const catalog = useActionSoundsListQuery();
  const [preview] = useActionSoundPreviewMutation();
  const [stopPreview] = useActionSoundStopPreviewMutation();
  const [isOpen, setIsOpen] = useState(false);
  const [error, setError] = useState<string>();
  const pending = useRef<Settings | undefined>(undefined);
  const mounted = useRef(true);
  const request = useRef(0);
  const volumeId = useId();
  const labelId = useId();
  const soundId = useId();

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      request.current += 1;
      pending.current = undefined;
      void stopPreview();
    };
  }, [stopPreview]);

  async function play(id: string, volumePercent: number) {
    const sequence = ++request.current;
    setError(undefined);
    const result = await preview({ id, volumePercent });
    if (mounted.current && sequence === request.current && result.error) setError("Не удалось прослушать звук. Проверьте аудиоустройство.");
  }

  async function stop() {
    const sequence = ++request.current;
    pending.current = undefined;
    setError(undefined);
    const result = await stopPreview();
    if (mounted.current && sequence === request.current && result.error) setError("Не удалось остановить предпрослушивание.");
  }

  function selectSound(id: string | null) {
    pending.current = undefined;
    onChange({ ...sound, id });
    setIsOpen(false);
    if (id === null) void stop();
  }

  function changeVolume(volumePercent: number) {
    const next = { ...sound, volumePercent };
    onChange(next);
    if (volumePercent === 0) {
      void stop();
      return;
    }
    pending.current = next;
  }

  function finishVolumeChange() {
    const latest = pending.current;
    pending.current = undefined;
    if (latest?.id && latest.volumePercent > 0) void play(latest.id, latest.volumePercent);
  }

  const name = catalog.data?.find(({ id }) => id === sound.id)?.name ?? "Без звука";
  return (
    <div className="ml-auto flex min-w-0 flex-col gap-1">
      <div className="flex flex-wrap items-center gap-2">
        <Label id={labelId} htmlFor={soundId}>Звук</Label>
        <Popover.Root open={isOpen} onOpenChange={setIsOpen}>
          <Popover.Trigger id={soundId} render={<Button variant="outline" />} role="combobox" aria-haspopup="dialog" aria-labelledby={labelId} className="min-w-32 justify-between gap-2">
            {name}<ChevronDown className="size-3" />
          </Popover.Trigger>
          <Popover.Portal>
            <Popover.Positioner sideOffset={4} className="z-50">
              <Popover.Popup aria-labelledby={labelId} className="max-h-80 w-64 max-w-[calc(100vw-1rem)] overflow-y-auto border bg-popover p-1 text-popover-foreground shadow-md">
                <Popover.Title className="sr-only">Звук</Popover.Title>
                <Button variant="ghost" aria-pressed={sound.id === null} onClick={() => selectSound(null)} className="w-full justify-start">Без звука</Button>
                {catalog.isLoading && <div role="status" aria-label="Загрузка звуков"><Skeleton className="h-8" /></div>}
                {catalog.isError && <div role="alert" className="p-2 text-xs">Не удалось загрузить звуки.<Button variant="outline" size="sm" onClick={() => catalog.refetch()}>Повторить</Button></div>}
                {catalog.data?.length === 0 && <p role="status" className="p-2 text-xs">Нет доступных звуков.</p>}
                {catalog.data?.map(({ id, name: itemName }) => (
                  <div key={id} className="flex items-center">
                    <Button variant="ghost" aria-pressed={sound.id === id} onClick={() => selectSound(id)} className={cn("min-w-0 flex-1 justify-start", sound.id === id && "bg-muted")}>
                      {itemName}{sound.id === id && <span aria-hidden="true" className="ml-auto">✓</span>}
                    </Button>
                    <Button variant="ghost" size="icon-sm" aria-label={`Прослушать ${itemName}`} onClick={() => { pending.current = undefined; void play(id, sound.volumePercent); }}><Play /></Button>
                  </div>
                ))}
              </Popover.Popup>
            </Popover.Positioner>
          </Popover.Portal>
        </Popover.Root>
        <div className="flex items-center gap-1">
          <label htmlFor={volumeId} className="sr-only">Громкость</label>
          <input
            id={volumeId} type="range" min="0" max="100" step="1"
            value={sound.volumePercent} disabled={sound.id === null}
            aria-valuetext={`${sound.volumePercent}%`} title={`${sound.volumePercent}%`}
            onChange={(event) => changeVolume(Number(event.target.value))}
            onPointerDown={(event) => {
              pending.current = sound;
              event.currentTarget.setPointerCapture?.(event.pointerId);
            }}
            onPointerUp={finishVolumeChange}
            onPointerCancel={() => { pending.current = undefined; }}
            onKeyUp={(event) => {
              if (["ArrowRight", "ArrowUp", "ArrowLeft", "ArrowDown", "Home", "End", "PageUp", "PageDown"].includes(event.key)) {
                finishVolumeChange();
              }
            }}
            onBlur={() => { pending.current = undefined; }}
            onKeyDown={(event) => {
              const values: Record<string, number> = {
                ArrowRight: sound.volumePercent + 1, ArrowUp: sound.volumePercent + 1,
                ArrowLeft: sound.volumePercent - 1, ArrowDown: sound.volumePercent - 1,
                Home: 0, End: 100, PageUp: sound.volumePercent + 10, PageDown: sound.volumePercent - 10,
              };
              if (event.key in values) {
                event.preventDefault();
                changeVolume(Math.max(0, Math.min(100, values[event.key])));
              }
            }}
            className="w-20 accent-primary focus-visible:outline focus-visible:outline-ring disabled:opacity-50"
          />
          <span aria-hidden="true" className="w-9 text-right font-mono text-xs tabular-nums">{sound.volumePercent}%</span>
        </div>
      </div>
      {error && <p role="alert" className="text-xs text-destructive">{error}</p>}
    </div>
  );
}
