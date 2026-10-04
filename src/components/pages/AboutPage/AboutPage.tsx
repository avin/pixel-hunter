import {
  Card,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { StackList } from "./StackList/StackList";

export function AboutPage() {
  return (
    <div className="flex flex-1 justify-center p-6">
      <Card className="w-full max-w-lg">
        <CardHeader>
          <CardTitle>About</CardTitle>
          <CardDescription>Current application stack.</CardDescription>
        </CardHeader>
        <StackList />
      </Card>
    </div>
  );
}
