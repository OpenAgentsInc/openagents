import { renderToCanvas, cleanup } from "@openagentsinc/storybook"

// Custom decorator to handle both HTML and Typed stories
export const decorators = [
  (Story, context) => {
    // Check if this is a Typed story by looking at the story's file path
    const isTypedStory = context.title?.startsWith("Typed/")
    
    if (isTypedStory) {
      // Return the Fx directly for Typed stories
      return Story()
    } else {
      // Return HTML element for regular stories
      return Story()
    }
  }
]

export const parameters = {
  actions: { argTypesRegex: "^on[A-Z].*" },
  controls: {
    matchers: {
      color: /(background|color)$/i,
      date: /Date$/i
    }
  },
  backgrounds: {
    default: "dark",
    values: [
      {
        name: "dark",
        value: "#000000"
      },
      {
        name: "light",
        value: "#ffffff"
      }
    ]
  }
}

// Export custom render function for Typed stories
export { renderToCanvas, cleanup }