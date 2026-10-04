import { useRef, useState } from "react";

type FieldErrors<TField extends string> = Partial<
  Record<TField, string | undefined>
>;

/**
 * An error is first shown after the field was changed and then blurred while
 * invalid, or after a submit attempt. Once shown, the field's error follows its
 * value live until the owning form remounts.
 */
export function useErrorReveal<TField extends string>() {
  const changedFields = useRef(new Set<TField>());
  const [revealedFields, setRevealedFields] = useState<ReadonlySet<TField>>(
    () => new Set(),
  );

  function reveal(fields: TField[]) {
    setRevealedFields((current) =>
      fields.every((field) => current.has(field))
        ? current
        : new Set([...current, ...fields]),
    );
  }

  return {
    visibleError(errors: FieldErrors<TField>, field: TField) {
      return revealedFields.has(field) ? errors[field] : undefined;
    },
    change(field: TField) {
      changedFields.current.add(field);
    },
    blur(field: TField, errors: FieldErrors<TField>) {
      if (errors[field] && changedFields.current.has(field)) {
        reveal([field]);
      }
    },
    /** Reveals every invalid field and returns the first one in `order`. */
    submit(errors: FieldErrors<TField>, order: readonly TField[]) {
      const invalid = order.filter((field) => errors[field]);
      reveal(invalid);
      return invalid[0];
    },
  };
}
