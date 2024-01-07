import { SimpleBuilder } from "@/Components/builder/SimpleBuilder";
import { NavLayout } from "@/Layouts/NavLayout";
import { usePage } from "@inertiajs/react";

function Builder() {
  const props = usePage().props as any
  return <SimpleBuilder errors={props.errors} />
}

Builder.layout = (page) => <NavLayout children={page} />

export default Builder
