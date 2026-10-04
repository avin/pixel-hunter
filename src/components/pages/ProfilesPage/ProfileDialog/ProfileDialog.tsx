import { useForm, useFormValidation, useFormWatch } from "@avinlab/react-form";
import { RefreshCw } from "lucide-react";
import { useId } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Field,
  FieldDescription,
  FieldError,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useErrorReveal } from "@/hooks/useErrorReveal";
import {
  useCreateProfileMutation,
  useListWindowedProcessesQuery,
  useUpdateProfileMutation,
} from "@/store/api";
import type { Profile, WindowedProcess } from "@/store/types";

interface Props {
  /** Edits this profile; creates a new one when omitted. */
  profile?: Profile;
  onClose: () => void;
}

interface Values {
  name: string;
  processName: string;
}

interface Errors {
  name?: string;
  processName?: string;
}

const FIELD_ORDER = ["name", "processName"] as const;

function validateProfile(values: Readonly<Values>): Errors {
  return {
    name: values.name.trim() ? undefined : "Введите имя профиля",
    processName: values.processName ? undefined : "Выберите процесс игры",
  };
}

export function ProfileDialog({ profile, onClose }: Props) {
  const form = useForm<Values>({
    name: profile?.name ?? "",
    processName: profile?.processName ?? "",
  });
  const name = useFormWatch(form, "name");
  const processName = useFormWatch(form, "processName");
  const { errors } = useFormValidation(form, validateProfile);
  const reveal = useErrorReveal<keyof Values>();

  const processes = useListWindowedProcessesQuery();
  const [createProfile, createState] = useCreateProfileMutation();
  const [updateProfile, updateState] = useUpdateProfileMutation();
  const isSaving = createState.isLoading || updateState.isLoading;

  const ids = {
    name: useId(),
    nameError: useId(),
    processLabel: useId(),
    process: useId(),
    processError: useId(),
  };
  const nameError = reveal.visibleError(errors, "name");
  const processError = reveal.visibleError(errors, "processName");
  const processOptions = withSavedProcess(
    processes.data ?? [],
    profile?.processName,
  );

  async function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const firstInvalid = reveal.submit(validateProfile(form.values), FIELD_ORDER);
    if (firstInvalid) {
      const fieldId = firstInvalid === "name" ? ids.name : ids.process;
      document.getElementById(fieldId)?.focus();
      return;
    }

    const input = {
      name: form.values.name.trim(),
      processName: form.values.processName,
    };
    const result = profile
      ? await updateProfile({ id: profile.id, input })
      : await createProfile(input);
    if (!("error" in result)) {
      onClose();
    }
  }

  return (
    <Dialog open onOpenChange={(isOpen) => !isOpen && onClose()}>
      <DialogContent className="sm:max-w-md">
        <form noValidate onSubmit={handleSubmit} className="contents">
          <DialogHeader>
            <DialogTitle>
              {profile ? "Редактирование профиля" : "Новый профиль"}
            </DialogTitle>
            <DialogDescription>
              Профиль привязан к процессу игры по имени исполняемого файла.
            </DialogDescription>
          </DialogHeader>

          <FieldGroup>
            <Field data-invalid={nameError ? true : undefined}>
              <FieldLabel htmlFor={ids.name}>Имя</FieldLabel>
              <Input
                id={ids.name}
                value={name}
                autoComplete="off"
                aria-invalid={nameError ? true : undefined}
                aria-describedby={nameError ? ids.nameError : undefined}
                onChange={(event) => {
                  form.setValue("name", event.currentTarget.value);
                  reveal.change("name");
                }}
                onBlur={() => reveal.blur("name", validateProfile(form.values))}
              />
              {nameError && <FieldError id={ids.nameError}>{nameError}</FieldError>}
            </Field>

            <Field data-invalid={processError ? true : undefined}>
              <FieldLabel id={ids.processLabel}>Процесс игры</FieldLabel>
              <div className="flex gap-2">
                <Select
                  value={processName || null}
                  onValueChange={(value) => {
                    form.setValue("processName", value ?? "");
                    reveal.change("processName");
                  }}
                >
                  <SelectTrigger
                    id={ids.process}
                    className="min-w-0 flex-1"
                    aria-labelledby={ids.processLabel}
                    aria-invalid={processError ? true : undefined}
                    aria-describedby={processError ? ids.processError : undefined}
                  >
                    <SelectValue
                      placeholder={
                        processes.isFetching ? "Загрузка…" : "Выберите процесс"
                      }
                    />
                  </SelectTrigger>
                  <SelectContent alignItemWithTrigger={false} className="max-w-[calc(100vw-2rem)]">
                    {processOptions.map((process) => (
                      <SelectItem key={process.exeName} value={process.exeName} className="*:last:min-w-0 *:last:flex-col *:last:items-start *:last:gap-0.5">
                        <span className="break-all font-mono">{process.exeName}</span>
                        <span className="whitespace-normal break-words text-muted-foreground">
                          {process.windowTitles.length > 0
                            ? process.windowTitles.join(", ")
                            : "не запущен"}
                        </span>
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                <Button
                  type="button"
                  variant="outline"
                  disabled={processes.isFetching}
                  onClick={() => processes.refetch()}
                >
                  <RefreshCw />
                  Обновить
                </Button>
              </div>
              <FieldDescription>
                В списке процессы, у которых есть видимое окно.
              </FieldDescription>
              {processError && (
                <FieldError id={ids.processError}>{processError}</FieldError>
              )}
            </Field>
          </FieldGroup>

          <DialogFooter>
            <DialogClose render={<Button type="button" variant="outline" />}>
              Отмена
            </DialogClose>
            <Button type="submit" disabled={isSaving}>
              {profile ? "Сохранить" : "Создать"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

/** Keeps the saved process selectable when it is not running right now. */
function withSavedProcess(
  processes: WindowedProcess[],
  savedProcessName: string | undefined,
): WindowedProcess[] {
  const isListed = processes.some(
    ({ exeName }) => exeName.toLowerCase() === savedProcessName?.toLowerCase(),
  );
  if (!savedProcessName || isListed) {
    return processes;
  }
  return [{ exeName: savedProcessName, windowTitles: [] }, ...processes];
}
