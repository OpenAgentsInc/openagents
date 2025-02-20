import { Link } from "react-router";
import { ShadComponents } from "~/components/library/shad";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "~/components/ui/breadcrumb";

export default function ShadcnPage() {
  return (
    <div className="container px-6 py-4">
      <div className="mx-auto">
        <Breadcrumb className="mb-6">
          <BreadcrumbList>
            <BreadcrumbItem>
              <BreadcrumbLink href="/components">Components</BreadcrumbLink>
            </BreadcrumbItem>
            <BreadcrumbSeparator />
            <BreadcrumbItem>
              <BreadcrumbPage>Shadcn UI</BreadcrumbPage>
            </BreadcrumbItem>
          </BreadcrumbList>
        </Breadcrumb>

        <div className="flex flex-col gap-6">
          <div>
            <h1 className="text-2xl font-bold mb-4">Shadcn UI Components</h1>
            <p className="text-muted-foreground">
              A comprehensive collection of beautifully designed, accessible,
              and customizable UI components.
            </p>
          </div>

          <ShadComponents />
        </div>
      </div>
    </div>
  );
}
