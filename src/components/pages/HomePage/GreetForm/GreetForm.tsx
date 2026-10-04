import { Loader2, Send } from "lucide-react";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useGreetMutation } from "@/store/api";

export function GreetForm() {
  const [name, setName] = useState("");
  const [greet, { data: greetMsg, isLoading, error }] = useGreetMutation();

  async function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!name.trim()) return;
    await greet({ name: name.trim() });
  }

  return (
    <Card className="w-full max-w-md">
      <CardHeader>
        <CardTitle>Welcome to Tauri + React</CardTitle>
        <CardDescription>
          shadcn/ui · Tailwind · Redux Toolkit · RTK Query
        </CardDescription>
      </CardHeader>

      <form className="contents" onSubmit={handleSubmit}>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="greet-input">Your name</Label>
            <Input
              id="greet-input"
              value={name}
              onChange={(event) => setName(event.currentTarget.value)}
              placeholder="Enter a name..."
              disabled={isLoading}
            />
          </div>

          {greetMsg && (
            <p className="rounded-lg bg-muted px-3 py-2 text-sm">{greetMsg}</p>
          )}

          {error && (
            <p className="rounded-lg bg-destructive/10 px-3 py-2 text-sm text-destructive">
              {error.message}
            </p>
          )}
        </CardContent>

        <CardFooter>
          <Button type="submit" disabled={isLoading || !name.trim()}>
            {isLoading ? <Loader2 className="animate-spin" /> : <Send />}
            Greet
          </Button>
        </CardFooter>
      </form>
    </Card>
  );
}
