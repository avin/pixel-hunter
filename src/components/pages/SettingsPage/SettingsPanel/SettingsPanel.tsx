import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";

export function SettingsPanel() {
  return (
    <Card className="w-full max-w-lg">
      <CardHeader>
        <CardTitle>Settings</CardTitle>
        <CardDescription>
          Example page routed via MemoryRouter.
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-3 text-sm text-muted-foreground">
        <p>
          Navigation lives in the sidebar. Routes are in-memory only — no URL
          changes in the WebView, which suits Tauri desktop apps.
        </p>
        <p>
          Add new pages under{" "}
          <code className="text-foreground">src/components/pages/</code>
          and register them in{" "}
          <code className="text-foreground">
            src/components/app/AppRouter/AppRouter.tsx
          </code>
          .
        </p>
      </CardContent>
    </Card>
  );
}
