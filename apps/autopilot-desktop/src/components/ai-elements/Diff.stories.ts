import { html } from "../../effuse/template/html"
import { Diff } from "./diff"

export default {
  title: "ai/Diff",
  component: Diff,
}

const sampleDiff = `--- a/src/main.ts
+++ b/src/main.ts
@@ -1,4 +1,4 @@
-console.log("Hello")
+console.log("Hello World")`

export const All = {
  render: () => html`
    <div class="flex flex-col gap-3">
      <div class="text-xs text-muted-foreground">Standard Diff</div>
      ${Diff({
        title: "src/main.ts",
        diff: sampleDiff,
        status: "applied",
      })}

      <div class="text-xs text-muted-foreground">Pending Diff</div>
      ${Diff({
        title: "src/components/header.ts",
        diff: sampleDiff,
        status: "pending",
      })}

      <div class="text-xs text-muted-foreground">No Status</div>
      ${Diff({
        title: "README.md",
        diff: sampleDiff,
        status: null,
      })}
    </div>
  `,
}
