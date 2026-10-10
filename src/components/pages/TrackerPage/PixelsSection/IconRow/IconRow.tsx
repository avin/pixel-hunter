import { useForm, useFormValidation, useFormWatch } from "@avinlab/react-form";
import { ImageOff, Trash2 } from "lucide-react";
import { useEffect, useId, useState } from "react";
import { Button } from "@/components/ui/button";
import { FieldError } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { TableCell, TableRow } from "@/components/ui/table";
import { useErrorReveal } from "@/hooks/useErrorReveal";
import { screenshotSrc } from "@/lib/screenshots";
import { useIconDeleteMutation, useIconUpdateMutation, useIconViewOpenMutation } from "@/store/api";
import { useAppSelector } from "@/store/hooks";
import type { Icon, IconCondition } from "@/store/types";

interface Props {
  profileId: string;
  trackerId: string;
  icon: Icon;
  screenshotsDir?: string;
}

interface Values { similarity: string }
const LABELS: Record<IconCondition, string> = { present: "Должна быть", absent: "Не должна быть" };
const FIELDS = ["similarity"] as const;
function validateSimilarity(values: Readonly<Values>) {
  const value = values.similarity.trim();
  const isValid = /^\d+$/.test(value) && Number(value) >= 1 && Number(value) <= 100;
  return { similarity: isValid ? undefined : "Введите целое число от 1 до 100" };
}

export function IconRow({ profileId, trackerId, icon, screenshotsDir }: Props) {
  const args = { profileId, trackerId, iconId: icon.id };
  const point = `(${icon.templateRect.x}, ${icon.templateRect.y})`;
  const form = useForm<Values>({ similarity: String(icon.similarityPercent) });
  const similarity = useFormWatch(form, "similarity");
  const { errors } = useFormValidation(form, validateSimilarity);
  const reveal = useErrorReveal<"similarity">();
  const error = reveal.visibleError(errors, "similarity");
  const errorId = useId();
  const similarityId = useId();
  const [hasThumbnailError, setHasThumbnailError] = useState(false);
  const [hasScreenshotError, setHasScreenshotError] = useState(false);
  const fullScreenshot = screenshotsDir && screenshotSrc(screenshotsDir, profileId, icon.id, "full");
  useEffect(() => {
    setHasScreenshotError(false);
    if (!fullScreenshot) return;
    // Probe once per file identity; editing the condition must not reload the full image.
    let probe: HTMLImageElement | undefined = new Image();
    probe.onload = () => { probe = undefined; };
    probe.onerror = () => {
      setHasScreenshotError(true);
      probe = undefined;
    };
    probe.src = fullScreenshot;
    return () => {
      if (probe) {
        probe.onload = null;
        probe.onerror = null;
      }
    };
  }, [fullScreenshot]);
  const [update] = useIconUpdateMutation();
  const [remove, removing] = useIconDeleteMutation();
  const [open] = useIconViewOpenMutation();
  const searchError = useAppSelector((state) => {
    const status = state.engine.iconErrors[icon.id];
    return status?.profileId === profileId && status.trackerId === trackerId ? status.error : undefined;
  });

  function saveSimilarity() {
    if (validateSimilarity(form.values).similarity || Number(form.values.similarity) === icon.similarityPercent) return;
    void update({ ...args, input: { condition: icon.condition, similarityPercent: Number(form.values.similarity) } });
  }

  return (
    <TableRow>
      <TableCell className="w-20">
        {screenshotsDir && !hasThumbnailError && !hasScreenshotError ? (
          <button type="button" aria-label={`Открыть снимок иконки ${point}`} className="block outline-none focus-visible:ring-3 focus-visible:ring-ring/50" onClick={(event) => {
            const bounds = event.currentTarget.getBoundingClientRect();
            const origin = event.detail === 0 ? { x: bounds.left + bounds.width / 2, y: bounds.top + bounds.height / 2 } : { x: event.clientX, y: event.clientY };
            void open({ ...args, origin });
          }}>
            <img src={screenshotSrc(screenshotsDir, profileId, icon.id, "thumbnail")} alt="" loading="lazy" width={64} height={32} className="h-8 w-16 border bg-muted object-contain" onError={() => setHasThumbnailError(true)} />
          </button>
        ) : (
          <div className="flex h-8 w-16 items-center justify-center border border-dashed bg-muted text-muted-foreground"><ImageOff aria-hidden className="size-4" /><span className="sr-only">Снимок недоступен</span></div>
        )}
      </TableCell>
      <TableCell className="w-20" title={`Образец ${icon.templateRect.width} × ${icon.templateRect.height}, ${point}`}>
        Иконка
      </TableCell>
      <TableCell colSpan={2}>
        <Select items={LABELS} value={icon.condition} onValueChange={(condition) => {
          if (condition && condition !== icon.condition) void update({ ...args, input: { condition, similarityPercent: icon.similarityPercent } });
        }}>
          <SelectTrigger size="sm" aria-label={`Условие иконки ${point}`}>
            <span className="grid">
              {Object.values(LABELS).map((label) => (
                <span key={label} aria-hidden className="invisible col-start-1 row-start-1">{label}</span>
              ))}
              <SelectValue className="col-start-1 row-start-1" />
            </span>
          </SelectTrigger>
          <SelectContent className="min-w-0">{Object.entries(LABELS).map(([value, label]) => <SelectItem key={value} value={value}>{label}</SelectItem>)}</SelectContent>
        </Select>
        {searchError && <p role="alert" className="mt-1 max-w-64 whitespace-normal text-destructive">{searchError}</p>}
      </TableCell>
      <TableCell>
        <div className="flex items-center justify-end gap-2">
          <label htmlFor={similarityId}>Сходство:</label>
          <div className="relative">
            <Input id={similarityId} type="number" inputMode="numeric" min={1} max={100} step={1} value={similarity} aria-label={`Сходство иконки ${point}, %`} aria-invalid={error ? true : undefined} aria-describedby={error ? errorId : undefined} className="h-7 w-20 pr-6 [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none" onChange={(event) => {
              form.setValue("similarity", event.currentTarget.value);
              reveal.change("similarity");
            }} onBlur={() => {
              reveal.blur("similarity", validateSimilarity(form.values));
              saveSimilarity();
            }} onKeyDown={(event) => {
              if (event.key === "Enter" && !reveal.submit(validateSimilarity(form.values), FIELDS)) saveSimilarity();
            }} />
            <span aria-hidden className="pointer-events-none absolute inset-y-0 right-2 flex items-center">%</span>
          </div>
        </div>
        {error && <FieldError id={errorId} className="mt-1 max-w-40 whitespace-normal">{error}</FieldError>}
      </TableCell>
      <TableCell className="w-10"><div className="flex justify-end"><Button variant="ghost" size="icon-sm" aria-label={`Удалить иконку ${point}`} disabled={removing.isLoading} onClick={() => void remove(args)}><Trash2 /></Button></div></TableCell>
    </TableRow>
  );
}
