import CanvasLayout from "@/components/canvas/CanvasLayout"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { DashboardLayout } from "@/Layouts/DashboardLayout"
import { Head } from "@inertiajs/react"

function Welcome() {
  return (
    <div className="w-full flex h-full items-center justify-center bg-background">
      <Head title="Welcome" />
      <CanvasLayout />
      <div className="pointer-events-none absolute select-none flex items-center justify-center">
        <Card className="-mt-6 px-6 py-2 bg-opacity-90 backdrop-blur-sm w-[400px]">
          <CardHeader>
            <CardTitle className="text-xl font-bold text-center">OpenAgents will return</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-center -mt-2">
              Hi! We are migrating to a new system. In the meantime, you can use our v2 system here:
            </p>
            <Button
              className="w-full mt-6 pointer-events-auto"
              variant="secondary"
              asChild
            >
              <a
                href="https://stage2.openagents.com"
                target="_blank"
                rel="noopener noreferrer"
              >
                Access previous version
              </a>
            </Button>
          </CardContent>
        </Card>
      </div>

    </div>
  );
}

Welcome.layout = (page) => <DashboardLayout children={page} />;

export default Welcome;
