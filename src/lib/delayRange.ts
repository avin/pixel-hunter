const MAX_MS = 4_294_967_295;

export function millisecondsError(value: string) {
  if (!/^\d+$/.test(value.trim())) {
    return "Введите целое неотрицательное число";
  }
  if (Number(value) > MAX_MS) {
    return "Слишком большое значение";
  }
  return undefined;
}

export function delayRangeError(min: string, max: string) {
  const error = millisecondsError(min) ?? millisecondsError(max);
  if (!error && Number(min) > Number(max)) {
    return "Минимум не может быть больше максимума";
  }
  return error;
}
