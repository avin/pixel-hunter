import { useForm, useFormValidation, useFormWatch } from "@avinlab/react-form";
import { ImageOff, Trash2 } from "lucide-react";
import { useId, useState } from "react";
import { Button } from "@/components/ui/button";
import { FieldError } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { TableCell, TableRow } from "@/components/ui/table";
import { useErrorReveal } from "@/hooks/useErrorReveal";
import { screenshotSrc } from "@/lib/screenshots";
import {
  usePixelDeleteMutation,
  usePixelUpdateMutation,
  usePixelViewOpenMutation,
} from "@/store/api";
import type { Pixel, PixelCondition, Rgb } from "@/store/types";

interface Props {
  profileId: string;
  trackerId: string;
  pixel: Pixel;
  screenshotsDir?: string;
}

interface Values {
  tolerance: string;
}

const CONDITION_LABELS: Record<PixelCondition, string> = {
  equals: "Равно",
  notEquals: "Не равно",
};
const FIELDS = ["tolerance"] as const;

function validateTolerance(values: Readonly<Values>) {
  const value = values.tolerance.trim();
  const isValid = /^\d+$/.test(value) && Number(value) <= 100;
  return { tolerance: isValid ? undefined : "Введите целое число от 0 до 100" };
}

function toHex({ r, g, b }: Rgb) {
  return `#${[r, g, b].map((part) => part.toString(16).padStart(2, "0")).join("")}`.toUpperCase();
}

export function PixelRow({ profileId, trackerId, pixel, screenshotsDir }: Props) {
  const pixelArgs = { profileId, trackerId, pixelId: pixel.id };
  const point = `(${pixel.x}, ${pixel.y})`;
  const hex = toHex(pixel.color);

  const form = useForm<Values>({ tolerance: String(pixel.tolerancePercent) });
  const tolerance = useFormWatch(form, "tolerance");
  const { errors } = useFormValidation(form, validateTolerance);
  const reveal = useErrorReveal<"tolerance">();
  const toleranceError = reveal.visibleError(errors, "tolerance");
  const toleranceErrorId = useId();
  const toleranceId = useId();

  const [hasThumbnailError, setHasThumbnailError] = useState(false);
  const [updatePixel] = usePixelUpdateMutation();
  const [deletePixel, deleteState] = usePixelDeleteMutation();
  const [openViewer] = usePixelViewOpenMutation();

  function saveTolerance() {
    const value = form.values.tolerance.trim();
    if (validateTolerance(form.values).tolerance || Number(value) === pixel.tolerancePercent) {
      return;
    }
    void updatePixel({
      ...pixelArgs,
      input: { condition: pixel.condition, tolerancePercent: Number(value) },
    });
  }

  return (
    <TableRow>
      <TableCell className="w-20">
        {screenshotsDir && !hasThumbnailError ? (
          <button
            type="button"
            aria-label={`Открыть снимок пикселя ${point}`}
            className="block outline-none focus-visible:ring-3 focus-visible:ring-ring/50"
            onClick={(event) => {
              const bounds = event.currentTarget.getBoundingClientRect();
              const origin = event.detail === 0
                ? { x: bounds.left + bounds.width / 2, y: bounds.top + bounds.height / 2 }
                : { x: event.clientX, y: event.clientY };
              void openViewer({ ...pixelArgs, origin });
            }}
          >
            <img
              src={screenshotSrc(screenshotsDir, profileId, pixel.id, "thumbnail")}
              alt=""
              loading="lazy"
              width={64}
              height={32}
              className="h-8 w-16 border bg-muted object-contain"
              onError={() => setHasThumbnailError(true)}
            />
          </button>
        ) : (
          <div className="flex h-8 w-16 items-center justify-center border border-dashed bg-muted text-muted-foreground">
            <ImageOff aria-hidden className="size-4" />
            <span className="sr-only">Снимок недоступен</span>
          </div>
        )}
      </TableCell>
      <TableCell className="w-20" title={`Пиксель ${point}`}>
        Пиксель
      </TableCell>
      <TableCell className="w-32">
        <Select
          items={CONDITION_LABELS}
          value={pixel.condition}
          onValueChange={(condition) => {
            if (condition && condition !== pixel.condition) {
              void updatePixel({
                ...pixelArgs,
                input: { condition, tolerancePercent: pixel.tolerancePercent },
              });
            }
          }}
        >
          <SelectTrigger size="sm" aria-label={`Условие пикселя ${point}`}>
            <span className="grid">
              {Object.values(CONDITION_LABELS).map((label) => (
                <span key={label} aria-hidden className="invisible col-start-1 row-start-1">{label}</span>
              ))}
              <SelectValue className="col-start-1 row-start-1" />
            </span>
          </SelectTrigger>
          <SelectContent alignItemWithTrigger={false} className="min-w-0">
            {Object.entries(CONDITION_LABELS).map(([value, label]) => (
              <SelectItem key={value} value={value}>
                {label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </TableCell>
      <TableCell>
        <div className="flex items-center gap-2">
          <span aria-hidden className="size-5 shrink-0 rounded-sm border" style={{ backgroundColor: hex }} />
          <span className="font-mono text-xs">{hex}</span>
        </div>
      </TableCell>
      <TableCell>
        <div>
          <div className="flex items-center justify-end gap-2">
            <label htmlFor={toleranceId}>Допуск:</label>
            <div className="relative">
              <Input
                id={toleranceId}
                type="number"
                inputMode="numeric"
                min={0}
                max={100}
                step={1}
                value={tolerance}
                aria-label={`Допуск пикселя ${point}, %`}
                aria-invalid={toleranceError ? true : undefined}
                aria-describedby={toleranceError ? toleranceErrorId : undefined}
                className="h-7 w-20 pr-6 [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
                onChange={(event) => {
                  form.setValue("tolerance", event.currentTarget.value);
                  reveal.change("tolerance");
                }}
                onBlur={() => {
                  reveal.blur("tolerance", validateTolerance(form.values));
                  saveTolerance();
                }}
                onKeyDown={(event) => {
                  if (
                    event.key === "Enter" &&
                    !reveal.submit(validateTolerance(form.values), FIELDS)
                  ) {
                    saveTolerance();
                  }
                }}
              />
              <span aria-hidden className="pointer-events-none absolute inset-y-0 right-2 flex items-center">%</span>
            </div>
          </div>
          {toleranceError && (
            <FieldError id={toleranceErrorId} className="mt-1 max-w-40 whitespace-normal">
              {toleranceError}
            </FieldError>
          )}
        </div>
      </TableCell>
      <TableCell className="w-10">
        <div className="flex justify-end">
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label={`Удалить пиксель ${point}`}
            disabled={deleteState.isLoading}
            onClick={() => void deletePixel(pixelArgs)}
          >
            <Trash2 />
          </Button>
        </div>
      </TableCell>
    </TableRow>
  );
}
