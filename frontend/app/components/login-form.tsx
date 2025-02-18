import { GalleryVerticalEnd, Mail } from "lucide-react";
import { Button } from "~/components/ui/button";
import { Input } from "~/components/ui/input";
import { Label } from "~/components/ui/label";
import { cn } from "~/lib/utils";

export function LoginForm({
  className,
  ...props
}: React.ComponentProps<"div">) {
  return (
    <div className={cn("flex flex-col gap-6", className)} {...props}>
      <form>
        <div className="flex flex-col gap-6">
          <div className="flex flex-col items-center gap-2">
            <span className="text-5xl select-none">⏻</span>
            <h1 className="text-xl font-bold">Welcome to OpenAgents</h1>
            <div className="text-center text-sm">
              Sign up or log in to continue
            </div>
          </div>
          <div className="flex flex-col gap-6">
            <div className="grid gap-3">
              <Label htmlFor="email">Email</Label>
              <Input
                id="email"
                type="email"
                placeholder="m@example.com"
                required
              />
            </div>
            <Button type="submit" className="w-full">
              <Mail className="mr-2 h-5 w-5" />
              Continue with email
            </Button>
          </div>
        </div>
      </form>
    </div>
  );
}
