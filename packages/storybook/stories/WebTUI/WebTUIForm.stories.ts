import type { Meta, StoryObj } from "@typed/storybook"
import { 
  WebTUIButton, 
  WebTUIInput, 
  WebTUITextarea, 
  Checkbox, 
  Switch, 
  Badge,
  RoundBox,
  Typography,
  Separator
} from "@openagentsinc/ui"
import { Fx } from "@typed/fx"
import { RenderEvent } from "@typed/dom/RenderEvent"
import { div } from "@typed/ui/hyperscript"

const meta: Meta = {
  title: "WebTUI/Complex Examples"
}

export default meta
type Story = StoryObj

export const LoginForm: Story = {
  render: () => {
    return RoundBox({
      style: "padding: 2rem; max-width: 400px; margin: 2rem auto;",
      children: [
        Typography({ variant: "h2", children: "Terminal Login", style: "text-align: center; margin-bottom: 1rem;" }),
        
        Separator({ style: "margin: 1rem 0;" }),
        
        div({ style: "margin-bottom: 1rem;" }, [
          Typography({ variant: "p", children: "Username:", style: "display: block; margin-bottom: 0.5rem;" }),
          WebTUIInput({ 
            placeholder: "Enter username", 
            size: "large",
            style: "width: 100%;"
          })
        ]),
        
        div({ style: "margin-bottom: 1rem;" }, [
          Typography({ variant: "p", children: "Password:", style: "display: block; margin-bottom: 0.5rem;" }),
          WebTUIInput({ 
            type: "password",
            placeholder: "Enter password", 
            size: "large",
            style: "width: 100%;"
          })
        ]),
        
        div({ style: "margin-bottom: 1rem; display: flex; gap: 1rem; align-items: center;" }, [
          Checkbox({ id: "remember", children: "Remember me" }),
          Badge({ children: "Optional", variant: "background2" })
        ]),
        
        div({ style: "margin-bottom: 1rem; display: flex; gap: 1rem; align-items: center;" }, [
          Switch({ id: "notifications", children: "Enable notifications" })
        ]),
        
        Separator({ style: "margin: 1rem 0;" }),
        
        div({ style: "display: flex; gap: 1rem; justify-content: center;" }, [
          WebTUIButton({ 
            children: "Cancel", 
            variant: "background2",
            box: "round"
          }),
          WebTUIButton({ 
            children: "Login", 
            variant: "foreground1",
            box: "round"
          })
        ])
      ]
    })
  }
}

export const TerminalInterface: Story = {
  render: () => {
    return div({ style: "max-width: 600px; margin: 2rem auto;" }, [
      RoundBox({
        style: "margin-bottom: 1rem;",
        children: [
          div({ style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem;" }, [
            Typography({ variant: "h3", children: "Terminal Interface" }),
            div({ style: "display: flex; gap: 0.5rem;" }, [
              Badge({ children: "Online", variant: "foreground1", capStart: "round", capEnd: "round" }),
              Badge({ children: "v1.0.0", variant: "background1" })
            ])
          ]),
          
          Separator({ style: "margin: 1rem 0;" }),
          
          div({ style: "margin-bottom: 1rem;" }, [
            Typography({ variant: "code", children: "$ system status", style: "display: block; margin-bottom: 0.5rem;" }),
            div({ style: "display: flex; gap: 1rem; flex-wrap: wrap;" }, [
              Badge({ children: "CPU: 45%", variant: "background1" }),
              Badge({ children: "RAM: 2.1GB", variant: "background2" }),
              Badge({ children: "Disk: 89%", variant: "foreground2" }),
              Badge({ children: "Network: OK", variant: "foreground1" })
            ])
          ]),
          
          div({ style: "margin-bottom: 1rem;" }, [
            Typography({ variant: "p", children: "Command:", style: "display: block; margin-bottom: 0.5rem;" }),
            WebTUIInput({ 
              placeholder: "Enter command...", 
              size: "large",
              style: "width: 100%; font-family: monospace;"
            })
          ]),
          
          div({ style: "display: flex; gap: 1rem; justify-content: flex-end;" }, [
            WebTUIButton({ 
              children: "Clear", 
              variant: "background3",
              size: "small"
            }),
            WebTUIButton({ 
              children: "Execute", 
              variant: "foreground0",
              box: "square"
            })
          ])
        ]
      })
    ])
  }
}