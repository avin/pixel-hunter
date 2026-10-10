import { useForm, useFormValidation, useFormWatch } from "@avinlab/react-form";
import { useId } from "react";
import { FieldError } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { useErrorReveal } from "@/hooks/useErrorReveal";
import { delayRangeError } from "@/lib/delayRange";
import type { DelayRange } from "@/store/types";

interface Props {
  label: string;
  range: DelayRange;
  onSave: (range: DelayRange) => void;
}

interface Values {
  min: string;
  max: string;
}

const FIELDS = ["delay"] as const;

function validateDelay(values: Readonly<Values>) {
  return { delay: delayRangeError(values.min, values.max) };
}

export function StepDelay({ label, range, onSave }: Props) {
  const form = useForm<Values>({ min: String(range.min), max: String(range.max) });
  const values = useFormWatch(form);
  const { errors } = useFormValidation(form, validateDelay);
  const reveal = useErrorReveal<"delay">();
  const error = reveal.visibleError(errors, "delay");
  const errorId = useId();

  function save() {
    const { min, max } = form.values;
    if (validateDelay(form.values).delay) return;
    if (Number(min) === range.min && Number(max) === range.max) return;
    onSave({ min: Number(min), max: Number(max) });
  }

  function input(field: keyof Values, bound: string) {
    return (
      <Input
        type="number"
        inputMode="numeric"
        min={0}
        step={1}
        value={values[field]}
        aria-label={`${label}: ${bound}, мс`}
        aria-invalid={error ? true : undefined}
        aria-describedby={error ? errorId : undefined}
        className="h-7 w-20"
        onChange={(event) => {
          form.setValue(field, event.currentTarget.value);
          reveal.change("delay");
        }}
        onKeyDown={(event) => {
          if (event.key === "Enter" && !reveal.submit(validateDelay(form.values), FIELDS)) {
            save();
          }
        }}
      />
    );
  }

  return (
    <fieldset
      className="flex flex-col gap-1"
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) {
          reveal.blur("delay", validateDelay(form.values));
          save();
        }
      }}
    >
      <legend className="sr-only">{label}, мс</legend>
      <div className="flex items-center gap-2 text-xs">
        {input("min", "минимум")}
        <span aria-hidden className="font-mono text-muted-foreground">–</span>
        {input("max", "максимум")}
      </div>
      {error && <FieldError id={errorId}>{error}</FieldError>}
    </fieldset>
  );
}
