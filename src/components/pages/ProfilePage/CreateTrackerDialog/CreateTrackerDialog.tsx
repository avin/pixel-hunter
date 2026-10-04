import { useForm, useFormValidation, useFormWatch } from "@avinlab/react-form";
import { useId } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Field, FieldError, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { useErrorReveal } from "@/hooks/useErrorReveal";
import { useCreateTrackerMutation } from "@/store/api";

interface Props {
  profileId: string;
  onClose: () => void;
}

interface Values {
  name: string;
}

interface Errors {
  name?: string;
}

const FIELD_ORDER = ["name"] as const;

function validateTracker(values: Readonly<Values>): Errors {
  return {
    name: values.name.trim() ? undefined : "Введите имя отслеживания",
  };
}

export function CreateTrackerDialog({ profileId, onClose }: Props) {
  const form = useForm<Values>({ name: "" });
  const name = useFormWatch(form, "name");
  const { errors } = useFormValidation(form, validateTracker);
  const reveal = useErrorReveal<keyof Values>();
  const [createTracker, { isLoading }] = useCreateTrackerMutation();
  const nameId = useId();
  const nameErrorId = useId();
  const nameError = reveal.visibleError(errors, "name");

  async function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (reveal.submit(validateTracker(form.values), FIELD_ORDER)) {
      document.getElementById(nameId)?.focus();
      return;
    }

    const result = await createTracker({
      profileId,
      name: form.values.name.trim(),
    });
    if (!("error" in result)) {
      onClose();
    }
  }

  return (
    <Dialog open onOpenChange={(isOpen) => !isOpen && onClose()}>
      <DialogContent>
        <form noValidate onSubmit={handleSubmit} className="contents">
          <DialogHeader>
            <DialogTitle>Новое отслеживание</DialogTitle>
          </DialogHeader>

          <Field data-invalid={nameError ? true : undefined}>
            <FieldLabel htmlFor={nameId}>Имя</FieldLabel>
            <Input
              id={nameId}
              value={name}
              autoComplete="off"
              aria-invalid={nameError ? true : undefined}
              aria-describedby={nameError ? nameErrorId : undefined}
              onChange={(event) => {
                form.setValue("name", event.currentTarget.value);
                reveal.change("name");
              }}
              onBlur={() => reveal.blur("name", validateTracker(form.values))}
            />
            {nameError && <FieldError id={nameErrorId}>{nameError}</FieldError>}
          </Field>

          <DialogFooter>
            <DialogClose render={<Button type="button" variant="outline" />}>
              Отмена
            </DialogClose>
            <Button type="submit" disabled={isLoading}>
              Создать
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
