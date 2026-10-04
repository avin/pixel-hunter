import { CardContent } from "@/components/ui/card";

const stack = [
  "Tauri 2",
  "React 19",
  "shadcn/ui + Tailwind CSS v4",
  "Redux Toolkit + RTK Query",
  "React Router (MemoryRouter)",
];

export function StackList() {
  return (
    <CardContent>
      <ul className="space-y-2 text-sm">
        {stack.map((item) => (
          <li
            key={item}
            className="rounded-lg bg-muted px-3 py-2 text-muted-foreground"
          >
            {item}
          </li>
        ))}
      </ul>
    </CardContent>
  );
}
