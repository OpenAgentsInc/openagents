import { html } from "@openagentsinc/effuse"

import { homePageTemplate } from "../../effuse-pages/home"

import type { Story } from "../types"

export const homeStories: ReadonlyArray<Story> = [
  {
    id: "home-introducing-autopilot",
    title: "Home/Introducing Autopilot",
    kind: "organism",
    render: () =>
      html`
        <div class="flex h-full min-h-0 w-full min-w-0 flex-col bg-bg-primary">
          ${homePageTemplate()}
        </div>
      `,
  },
]
