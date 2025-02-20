import { Link } from "react-router";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "~/components/ui/breadcrumb";

export default function ComponentsPage() {
  return (
    <div className="container px-6 py-4">
      <div className="mx-auto">
        <Breadcrumb className="mb-6">
          <BreadcrumbList>
            <BreadcrumbItem>
              <BreadcrumbPage>Components</BreadcrumbPage>
            </BreadcrumbItem>
          </BreadcrumbList>
        </Breadcrumb>

        <div className="flex flex-col gap-6">
          <div>
            <h1 className="text-2xl font-bold mb-4">Components</h1>
            <p className="text-muted-foreground">
              Explore our collection of interactive components and UI elements.
            </p>
          </div>

          <div className="grid gap-4">
            <Link
              to="/components/thinking"
              className="group p-4 border rounded-lg hover:border-primary transition-colors"
            >
              <h2 className="text-lg font-semibold group-hover:text-primary transition-colors">
                Chain of Thought
              </h2>
              <p className="text-muted-foreground">
                Visualize AI thinking process with animated text streaming and
                progress tracking.
              </p>
            </Link>

            <Link
              to="/components/shadcn"
              className="group p-4 border rounded-lg hover:border-primary transition-colors"
            >
              <h2 className="text-lg font-semibold group-hover:text-primary transition-colors">
                Shadcn UI Components
              </h2>
              <p className="text-muted-foreground">
                Collection of beautifully designed, accessible, and customizable
                UI components.
              </p>
            </Link>
          </div>
        </div>
      </div>
    </div>
  );
}
