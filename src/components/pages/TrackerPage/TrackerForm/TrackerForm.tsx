import { useForm, useFormValidation, useFormWatch } from "@avinlab/react-form";
import { useEffect, useId, useRef, useState } from "react";
import { useBlocker } from "react-router-dom";
import { useWorkspaceNavigation } from "@/components/app/AppLayout/workspaceNavigation";
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
import { delayRangeError, millisecondsError } from "@/lib/delayRange";
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

function validateTracker(values: Readonly<Values>): Errors {
  return {
    name: values.name.trim() ? undefined : "Введите имя отслеживания",
    preActionDelay: delayRangeError(values.delayMin, values.delayMax),
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
  const baseline = useRef(form.values);
  const isSaving = useRef(false);
  const [saveError, setSaveError] = useState<string>();
  const { setIsLeaving } = useWorkspaceNavigation();
  const isDirty = (Object.keys(values) as (keyof Values)[]).some((key) => values[key] !== baseline.current[key]);
  const blocker = useBlocker(({ currentLocation, nextLocation }) => currentLocation.pathname !== nextLocation.pathname && (isDirty || isSaving.current));
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
      setSaveError(undefined);
      form.setValue(field, event.currentTarget.value);
      reveal.change(errorField);
    };
  }

  function blurField(errorField: ErrorField) {
    reveal.blur(errorField, validateTracker(form.values));
  }

  useEffect(() => {
    if (blocker.state !== "blocked" || isSaving.current) return;
    const transition = blocker;
    const firstInvalid = reveal.submit(validateTracker(form.values), FIELD_ORDER);
    if (firstInvalid) {
      const fieldId = {
        name: ids.name,
        preActionDelay: ids.delayMin,
        resumeDelay: ids.resumeDelay,
      }[firstInvalid];
      document.getElementById(fieldId)?.focus();
      transition.reset();
      return;
    }
    isSaving.current = true;
    setIsLeaving(true);
    setSaveError(undefined);
    const draft = form.values;
    void updateTracker({
      profileId,
      trackerId: tracker.id,
      input: {
        name: draft.name.trim(),
        preActionDelayMs: { min: Number(draft.delayMin), max: Number(draft.delayMax) },
        resumeDelayMs: Number(draft.resumeDelay),
      },
    }).then((result) => {
      isSaving.current = false;
      setIsLeaving(false);
      if ("error" in result) {
        const error = result.error;
        setSaveError(error && "message" in error ? String(error.message) : "Не удалось сохранить настройки");
        transition.reset();
      } else {
        baseline.current = draft;
        transition.proceed();
      }
    });
  }, [blocker]);

  return (
    <form noValidate onSubmit={(event) => event.preventDefault()} aria-label="Настройки отслеживания" className="shrink-0 border-b p-3">
      <fieldset disabled={isLoading} className="min-w-0">
      <FieldGroup className="grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)_minmax(0,1fr)] items-start gap-3">
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
          <div className="grid grid-cols-2 gap-2">
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

      </fieldset>
      {isLoading && <p role="status" className="mt-2 text-muted-foreground">Сохранение…</p>}
      {saveError && <p role="alert" className="mt-2 text-destructive">{saveError}</p>}
    </form>
  );
}
