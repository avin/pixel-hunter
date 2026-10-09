import { useForm, useFormValidation, useFormWatch } from "@avinlab/react-form";
import { useEffect, useId, useRef, useState } from "react";
import { useBlocker } from "react-router-dom";
import { useWorkspaceNavigation } from "@/components/app/AppLayout/workspaceNavigation";
import {
  Field,
  FieldError,
  FieldGroup,
  FieldLabel,
  FieldLegend,
  FieldSet,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { delayRangeError, millisecondsError } from "@/lib/delayRange";
import { useUpdateTrackerMutation } from "@/store/api";
import type { Tracker } from "@/store/types";
import { FieldInfo } from "./FieldInfo/FieldInfo";

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

function trackerInput(values: Readonly<Values>) {
  return {
    name: values.name.trim(),
    preActionDelayMs: { min: Number(values.delayMin), max: Number(values.delayMax) },
    resumeDelayMs: Number(values.resumeDelay),
  };
}

function settingsKey(values: Readonly<Values>) {
  return JSON.stringify(trackerInput(values));
}

function firstInvalidField(values: Readonly<Values>) {
  const errors = validateTracker(values);
  return FIELD_ORDER.find((field) => errors[field]);
}

export function TrackerForm({ profileId, tracker }: Props) {
  const form = useForm<Values>({
    name: tracker.name,
    delayMin: String(tracker.preActionDelayMs.min),
    delayMax: String(tracker.preActionDelayMs.max),
    resumeDelay: String(tracker.resumeDelayMs),
  });
  const values = useFormWatch(form);
  const baseline = useRef(settingsKey(form.values));
  const pendingSave = useRef<Promise<boolean> | null>(null);
  const lastChange = useRef(0);
  const failedDraft = useRef<Readonly<Values> | null>(null);
  const leaving = useRef(false);
  const mounted = useRef(true);
  const [isSaving, setIsSaving] = useState(false);
  const [saveError, setSaveError] = useState<string>();
  const { isLeaving, setIsLeaving } = useWorkspaceNavigation();
  const blocker = useBlocker(({ currentLocation, nextLocation }) =>
    currentLocation.pathname !== nextLocation.pathname &&
    (settingsKey(form.values) !== baseline.current ||
      firstInvalidField(form.values) !== undefined ||
      pendingSave.current !== null),
  );
  const { errors } = useFormValidation(form, validateTracker);
  const [revealed, setRevealed] = useState<ReadonlySet<ErrorField>>(() => new Set());
  const [updateTracker] = useUpdateTrackerMutation();

  const ids = {
    name: useId(),
    nameError: useId(),
    delayMin: useId(),
    delayMax: useId(),
    delayError: useId(),
    resumeDelay: useId(),
    resumeDelayError: useId(),
  };
  const nameError = revealed.has("name") ? errors.name : undefined;
  const delayError = revealed.has("preActionDelay") ? errors.preActionDelay : undefined;
  const resumeDelayError = revealed.has("resumeDelay") ? errors.resumeDelay : undefined;

  function setField(field: keyof Values, errorField: ErrorField) {
    return (event: React.ChangeEvent<HTMLInputElement>) => {
      setSaveError(undefined);
      failedDraft.current = null;
      lastChange.current = Date.now();
      form.setValue(field, event.currentTarget.value);
      setRevealed((current) => current.has(errorField) ? current : new Set([...current, errorField]));
    };
  }

  function saveDraft(): Promise<boolean> {
    if (pendingSave.current) return pendingSave.current;
    const draft = form.values;
    if (firstInvalidField(draft)) return Promise.resolve(false);
    const key = settingsKey(draft);
    if (key === baseline.current) return Promise.resolve(true);
    setIsSaving(true);
    setSaveError(undefined);
    const request = updateTracker({ profileId, trackerId: tracker.id, input: trackerInput(draft) })
      .then((result) => {
        if ("error" in result) {
          failedDraft.current = draft;
          const error = result.error;
          if (mounted.current) setSaveError(error && "message" in error ? String(error.message) : "Не удалось сохранить настройки");
          return false;
        }
        baseline.current = key;
        return true;
      }).finally(() => {
        pendingSave.current = null;
        if (mounted.current) setIsSaving(false);
      });
    pendingSave.current = request;
    return request;
  }

  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);

  useEffect(() => {
    if (leaving.current || pendingSave.current || failedDraft.current === values ||
        settingsKey(values) === baseline.current || firstInvalidField(values)) return;
    const timeout = window.setTimeout(() => { void saveDraft(); }, Math.max(0, 500 - (Date.now() - lastChange.current)));
    return () => window.clearTimeout(timeout);
  }, [values, isSaving, blocker.state]);

  useEffect(() => {
    if (blocker.state !== "blocked" || leaving.current) return;
    const transition = blocker;
    const firstInvalid = firstInvalidField(form.values);
    if (firstInvalid) {
      setRevealed(new Set(FIELD_ORDER));
      const fieldId = {
        name: ids.name,
        preActionDelay: ids.delayMin,
        resumeDelay: ids.resumeDelay,
      }[firstInvalid];
      document.getElementById(fieldId)?.focus();
      transition.reset();
      return;
    }
    leaving.current = true;
    setIsLeaving(true);
    setSaveError(undefined);
    void (async () => {
      const previousSucceeded = pendingSave.current ? await pendingSave.current : true;
      const succeeded = previousSucceeded && await saveDraft();
      if (!mounted.current) return;
      leaving.current = false;
      setIsLeaving(false);
      if (!succeeded) {
        transition.reset();
      } else {
        transition.proceed();
      }
    })();
  }, [blocker]);

  return (
    <form noValidate onSubmit={(event) => event.preventDefault()} aria-label="Настройки отслеживания" className="shrink-0 border-b p-3">
      <fieldset disabled={isLeaving} className="min-w-0">
      <FieldGroup className="grid grid-cols-1 items-start gap-3 sm:grid-cols-3">
        <Field className="gap-1" data-invalid={nameError ? true : undefined}>
          <FieldLabel htmlFor={ids.name}>Имя</FieldLabel>
          <Input
            id={ids.name}
            value={values.name}
            autoComplete="off"
            aria-invalid={nameError ? true : undefined}
            aria-describedby={nameError ? ids.nameError : undefined}
            onChange={setField("name", "name")}
          />
          {nameError && <FieldError id={ids.nameError}>{nameError}</FieldError>}
        </Field>

        <FieldSet className="gap-1">
          <FieldLegend variant="label" className="mb-1">
            <span className="flex items-center gap-1">
              Задержка перед действием, мс
              <FieldInfo label="Задержка перед действием">
                Перед действием выдерживается случайная пауза из этого диапазона.
              </FieldInfo>
            </span>
          </FieldLegend>
          <div className="grid grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] items-center gap-2">
            <Field data-invalid={delayError ? true : undefined}>
              <FieldLabel htmlFor={ids.delayMin} className="sr-only">Минимум</FieldLabel>
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
            <span aria-hidden="true" className="font-mono text-muted-foreground">—</span>
            <Field data-invalid={delayError ? true : undefined}>
              <FieldLabel htmlFor={ids.delayMax} className="sr-only">Максимум</FieldLabel>
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

        <Field className="gap-1" data-invalid={resumeDelayError ? true : undefined}>
          <div className="flex items-center gap-1">
          <FieldLabel htmlFor={ids.resumeDelay}>
            Пауза до возобновления опроса, мс
          </FieldLabel>
          <FieldInfo label="Пауза до возобновления опроса">
            Сколько ждать после действия, прежде чем снова проверять пиксели.
          </FieldInfo>
          </div>
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
          />
          {resumeDelayError && (
            <FieldError id={ids.resumeDelayError}>{resumeDelayError}</FieldError>
          )}
        </Field>
      </FieldGroup>

      </fieldset>
      {(isSaving || isLeaving) && <p role="status" className="mt-2 text-muted-foreground">Сохранение…</p>}
      {saveError && <p role="alert" className="mt-2 text-destructive">{saveError}</p>}
    </form>
  );
}
