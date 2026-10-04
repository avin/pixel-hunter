import { useForm, useFormValidation, useFormWatch } from "@avinlab/react-form";
import { useId } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Field,
  FieldDescription,
  FieldError,
  FieldGroup,
  FieldLabel,
  FieldLegend,
  FieldSet,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { useErrorReveal } from "@/hooks/useErrorReveal";
import { useUpdateTrackerMutation } from "@/store/api";
import type { Tracker } from "@/store/types";

interface Props {
  profileId: string;
  tracker: Tracker;
}

interface Values {
  name: string;
  delayMin: string;
  delayMax: string;
  resumeDelay: string;
}

type ErrorField = "name" | "preActionDelay" | "resumeDelay";
type Errors = Partial<Record<ErrorField, string>>;

const FIELD_ORDER: readonly ErrorField[] = ["name", "preActionDelay", "resumeDelay"];
const MAX_MS = 4_294_967_295;

function millisecondsError(value: string) {
  if (!/^\d+$/.test(value.trim())) {
    return "Введите целое неотрицательное число";
  }
  if (Number(value) > MAX_MS) {
    return "Слишком большое значение";
  }
  return undefined;
}

function validateTracker(values: Readonly<Values>): Errors {
  const delayError =
    millisecondsError(values.delayMin) ?? millisecondsError(values.delayMax);
  const isRangeReversed =
    !delayError && Number(values.delayMin) > Number(values.delayMax);

  return {
    name: values.name.trim() ? undefined : "Введите имя отслеживания",
    preActionDelay: isRangeReversed
      ? "Минимум не может быть больше максимума"
      : delayError,
    resumeDelay: millisecondsError(values.resumeDelay),
  };
}

export function TrackerForm({ profileId, tracker }: Props) {
  const form = useForm<Values>({
    name: tracker.name,
    delayMin: String(tracker.preActionDelayMs.min),
    delayMax: String(tracker.preActionDelayMs.max),
    resumeDelay: String(tracker.resumeDelayMs),
  });
  const values = useFormWatch(form);
  const { errors } = useFormValidation(form, validateTracker);
  const reveal = useErrorReveal<ErrorField>();
  const [updateTracker, { isLoading }] = useUpdateTrackerMutation();

  const ids = {
    name: useId(),
    nameError: useId(),
    delayMin: useId(),
    delayMax: useId(),
    delayError: useId(),
    resumeDelay: useId(),
    resumeDelayError: useId(),
  };
  const nameError = reveal.visibleError(errors, "name");
  const delayError = reveal.visibleError(errors, "preActionDelay");
  const resumeDelayError = reveal.visibleError(errors, "resumeDelay");

  function setField(field: keyof Values, errorField: ErrorField) {
    return (event: React.ChangeEvent<HTMLInputElement>) => {
      form.setValue(field, event.currentTarget.value);
      reveal.change(errorField);
    };
  }

  function blurField(errorField: ErrorField) {
    reveal.blur(errorField, validateTracker(form.values));
  }

  async function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const firstInvalid = reveal.submit(validateTracker(form.values), FIELD_ORDER);
    if (firstInvalid) {
      const fieldId = {
        name: ids.name,
        preActionDelay: ids.delayMin,
        resumeDelay: ids.resumeDelay,
      }[firstInvalid];
      document.getElementById(fieldId)?.focus();
      return;
    }

    const result = await updateTracker({
      profileId,
      trackerId: tracker.id,
      input: {
        name: form.values.name.trim(),
        preActionDelayMs: {
          min: Number(form.values.delayMin),
          max: Number(form.values.delayMax),
        },
        resumeDelayMs: Number(form.values.resumeDelay),
      },
    });
    if (!("error" in result)) {
      toast.success("Отслеживание сохранено");
    }
  }

  return (
    <form noValidate onSubmit={handleSubmit} className="flex max-w-md flex-col gap-6 p-6">
      <FieldGroup>
        <Field data-invalid={nameError ? true : undefined}>
          <FieldLabel htmlFor={ids.name}>Имя</FieldLabel>
          <Input
            id={ids.name}
            value={values.name}
            autoComplete="off"
            aria-invalid={nameError ? true : undefined}
            aria-describedby={nameError ? ids.nameError : undefined}
            onChange={setField("name", "name")}
            onBlur={() => blurField("name")}
          />
          {nameError && <FieldError id={ids.nameError}>{nameError}</FieldError>}
        </Field>

        <FieldSet
          onBlur={(event) => {
            if (!event.currentTarget.contains(event.relatedTarget)) {
              blurField("preActionDelay");
            }
          }}
        >
          <FieldLegend variant="label">Задержка перед действием, мс</FieldLegend>
          <FieldDescription>
            Перед действием выдерживается случайная пауза из этого диапазона.
          </FieldDescription>
          <div className="grid grid-cols-2 gap-3">
            <Field data-invalid={delayError ? true : undefined}>
              <FieldLabel htmlFor={ids.delayMin}>Минимум</FieldLabel>
              <Input
                id={ids.delayMin}
                type="number"
                inputMode="numeric"
                min={0}
                step={1}
                value={values.delayMin}
                aria-invalid={delayError ? true : undefined}
                aria-describedby={delayError ? ids.delayError : undefined}
                onChange={setField("delayMin", "preActionDelay")}
              />
            </Field>
            <Field data-invalid={delayError ? true : undefined}>
              <FieldLabel htmlFor={ids.delayMax}>Максимум</FieldLabel>
              <Input
                id={ids.delayMax}
                type="number"
                inputMode="numeric"
                min={0}
                step={1}
                value={values.delayMax}
                aria-invalid={delayError ? true : undefined}
                aria-describedby={delayError ? ids.delayError : undefined}
                onChange={setField("delayMax", "preActionDelay")}
              />
            </Field>
          </div>
          {delayError && <FieldError id={ids.delayError}>{delayError}</FieldError>}
        </FieldSet>

        <Field data-invalid={resumeDelayError ? true : undefined}>
          <FieldLabel htmlFor={ids.resumeDelay}>
            Пауза до возобновления опроса, мс
          </FieldLabel>
          <Input
            id={ids.resumeDelay}
            type="number"
            inputMode="numeric"
            min={0}
            step={1}
            value={values.resumeDelay}
            aria-invalid={resumeDelayError ? true : undefined}
            aria-describedby={resumeDelayError ? ids.resumeDelayError : undefined}
            onChange={setField("resumeDelay", "resumeDelay")}
            onBlur={() => blurField("resumeDelay")}
          />
          <FieldDescription>
            Сколько ждать после действия, прежде чем снова проверять пиксели.
          </FieldDescription>
          {resumeDelayError && (
            <FieldError id={ids.resumeDelayError}>{resumeDelayError}</FieldError>
          )}
        </Field>
      </FieldGroup>

      <div>
        <Button type="submit" disabled={isLoading}>
          Сохранить
        </Button>
      </div>
    </form>
  );
}
