import { SimpleBuilder } from "@/Components/builder/SimpleBuilder";
import { NavLayout } from "@/Layouts/NavLayout";

function Builder() {
  return <SimpleBuilder />
}

Builder.layout = (page) => <NavLayout children={page} />

export default Builder
