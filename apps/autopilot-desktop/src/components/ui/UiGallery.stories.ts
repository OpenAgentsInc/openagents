import { html } from "../../effuse/template/html"
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "./accordion"
import {
  Alert,
  AlertDescription,
  AlertTitle,
} from "./alert"
import {
  Avatar,
  AvatarBadge,
  AvatarFallback,
  AvatarGroup,
  AvatarGroupCount,
  AvatarImage,
} from "./avatar"
import { Badge } from "./badge"
import {
  Button,
} from "./button"
import {
  ButtonGroup,
  ButtonGroupButton,
  ButtonGroupSeparator,
  ButtonGroupText,
} from "./button-group"
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "./card"
import {
  Carousel,
  CarouselContent,
  CarouselItem,
  CarouselNext,
  CarouselPrevious,
} from "./carousel"
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "./collapsible"
import {
  Command,
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
  CommandShortcut,
} from "./command"
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogOverlay,
  DialogPortal,
  DialogTitle,
  DialogTrigger,
} from "./dialog"
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuPortal,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuShortcut,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from "./dropdown-menu"
import {
  HoverCard,
  HoverCardContent,
  HoverCardTrigger,
} from "./hover-card"
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupInput,
  InputGroupText,
  InputGroupTextarea,
} from "./input-group"
import { Input } from "./input"
import {
  Popover,
  PopoverAnchor,
  PopoverContent,
  PopoverDescription,
  PopoverHeader,
  PopoverTitle,
  PopoverTrigger,
} from "./popover"
import { Progress } from "./progress"
import { ScrollArea, ScrollBar } from "./scroll-area"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectLabel,
  SelectScrollDownButton,
  SelectScrollUpButton,
  SelectSeparator,
  SelectTrigger,
  SelectValue,
} from "./select"
import { Separator } from "./separator"
import { Switch } from "./switch"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./tabs"
import { Textarea } from "./textarea"
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "./tooltip"
import { mountUiRuntime } from "./runtime"
import { cx } from "./utils"

export default {
  title: "ui/Gallery",
}

export const Basics = {
  render: () => html`
    <div class="flex flex-col gap-6">
      <div class="space-y-2">
        <div class="text-xs uppercase text-muted-foreground">Buttons</div>
        <div class="flex flex-wrap gap-2">
          ${Button({ children: "Primary" })}
          ${Button({ variant: "secondary", children: "Secondary" })}
          ${Button({ variant: "outline", children: "Outline" })}
          ${Button({ variant: "ghost", children: "Ghost" })}
          ${Button({ variant: "destructive", children: "Destructive" })}
          ${Button({ variant: "link", children: "Link" })}
        </div>
        <div class="flex flex-wrap gap-2">
          ${Button({ size: "xs", children: "XS" })}
          ${Button({ size: "sm", children: "SM" })}
          ${Button({ size: "default", children: "MD" })}
          ${Button({ size: "lg", children: "LG" })}
          ${Button({ size: "icon", children: "◎" })}
        </div>
      </div>

      <div class="space-y-2">
        <div class="text-xs uppercase text-muted-foreground">Button Group + Badges</div>
        ${ButtonGroup({
          children: html`
            ${ButtonGroupText({ children: "Status" })}
            ${ButtonGroupSeparator({})}
            ${ButtonGroupButton({ children: "One" })}
            ${ButtonGroupButton({ children: "Two" })}
            ${ButtonGroupButton({ children: "Three" })}
          `,
        })}
        <div class="flex flex-wrap gap-2">
          ${Badge({ children: "Default" })}
          ${Badge({ variant: "secondary", children: "Secondary" })}
          ${Badge({ variant: "outline", children: "Outline" })}
        </div>
      </div>

      <div class="space-y-2">
        <div class="text-xs uppercase text-muted-foreground">Alert + Avatar</div>
        ${Alert({
          children: html`
            <span class="size-4">!</span>
            ${AlertTitle({ children: "Heads up" })}
            ${AlertDescription({ children: "This is a baseline alert with description." })}
          `,
        })}
        <div class="flex items-center gap-4">
          ${Avatar({
            children: html`
              ${AvatarImage({ src: "https://placehold.co/64x64" })}
              ${AvatarFallback({ children: "OA" })}
            `,
          })}
          ${Avatar({
            children: html`${AvatarFallback({ children: "AP" })}`,
          })}
        </div>
      </div>
    </div>
  `,
}

export const Forms = {
  render: () => html`
    <div class="flex flex-col gap-6">
      <div class="space-y-2">
        <div class="text-xs uppercase text-muted-foreground">Inputs</div>
        ${Input({ placeholder: "Search" })}
        ${Textarea({ placeholder: "Describe the task" })}
        <div class="flex items-center gap-3">
          <span class="text-sm text-muted-foreground">Notifications</span>
          ${Switch({ checked: true })}
        </div>
      </div>

      <div class="space-y-2">
        <div class="text-xs uppercase text-muted-foreground">Input Group</div>
        ${InputGroup({
          children: html`
            ${InputGroupAddon({ children: "$" })}
            ${InputGroupInput({ placeholder: "Amount" })}
            ${InputGroupButton({ size: "sm", children: "Send" })}
          `,
        })}
        ${InputGroup({
          children: html`
            ${InputGroupText({ children: "Notes" })}
            ${InputGroupTextarea({ placeholder: "Optional details" })}
          `,
        })}
      </div>

      <div class="space-y-2">
        <div class="text-xs uppercase text-muted-foreground">Select + Progress</div>
        ${Select({
          children: html`
            ${SelectTrigger({ children: html`${SelectValue({ children: "Default" })}` })}
            ${SelectContent({ state: "open", children: html`
              ${SelectItem({ value: "default", selected: true, children: "Default" })}
              ${SelectItem({ value: "fast", children: "Fast" })}
              ${SelectItem({ value: "safe", children: "Safe" })}
            ` })}
          `,
        })}
        ${Progress({ value: 64 })}
      </div>
    </div>
  `,
}

export const Layout = {
  render: () => html`
    <div class="flex flex-col gap-6">
      ${Card({
        className: "max-w-md",
        children: html`
          ${CardHeader({
            children: html`
              ${CardTitle({ children: "Card Title" })}
              ${CardDescription({ children: "Supporting description text." })}
            `,
          })}
          ${CardContent({ children: "Card body content." })}
          ${CardFooter({ children: html`${Button({ size: "sm", children: "Action" })}` })}
        `,
      })}

      ${Tabs({
        defaultValue: "first",
        children: html`
          ${TabsList({ children: html`
            ${TabsTrigger({ value: "first", active: true, children: "First" })}
            ${TabsTrigger({ value: "second", children: "Second" })}
          ` })}
          ${TabsContent({ value: "first", active: true, children: "First tab content." })}
          ${TabsContent({ value: "second", children: "Second tab content." })}
        `,
      })}

      ${Accordion({
        type: "single",
        children: html`
          ${AccordionItem({
            children: html`
              ${AccordionTrigger({ children: "Accordion Item" })}
              ${AccordionContent({ state: "open", children: "Accordion content goes here." })}
            `,
          })}
        `,
      })}

      ${Collapsible({
        state: "open",
        children: html`
          ${CollapsibleTrigger({ children: "Collapsible Trigger" })}
          ${CollapsibleContent({ children: "Collapsible content" })}
        `,
      })}

      ${Separator({})}

      ${ScrollArea({
        className: "h-24 rounded-md border",
        children: html`
          <div class="space-y-2 p-2 text-sm">
            <p>Scrollable line one.</p>
            <p>Scrollable line two.</p>
            <p>Scrollable line three.</p>
            <p>Scrollable line four.</p>
          </div>
          ${ScrollBar({ orientation: "vertical" })}
        `,
      })}

      ${Carousel({
        className: "w-full max-w-md",
        children: html`
          ${CarouselContent({ children: html`
            ${CarouselItem({ children: html`<div class="rounded-md border bg-muted p-6 text-sm">Slide 1</div>` })}
            ${CarouselItem({ children: html`<div class="rounded-md border bg-muted p-6 text-sm">Slide 2</div>` })}
          ` })}
          ${CarouselPrevious({})}
          ${CarouselNext({})}
        `,
      })}
    </div>
  `,
}

export const Overlays = {
  render: () => html`
    <div class="flex flex-col gap-6">
      ${DropdownMenu({
        children: html`
          ${DropdownMenuTrigger({ children: Button({ variant: "outline", children: "Open menu" }) })}
          ${DropdownMenuContent({ state: "open", children: html`
            ${DropdownMenuLabel({ children: "Actions" })}
            ${DropdownMenuItem({ children: "Rename" })}
            ${DropdownMenuItem({ children: "Duplicate" })}
            ${DropdownMenuSeparator({})}
            ${DropdownMenuItem({ children: "Delete" })}
          ` })}
        `,
      })}

      ${Popover({
        children: html`
          ${PopoverTrigger({ children: Button({ variant: "outline", children: "Open popover" }) })}
          ${PopoverContent({ state: "open", children: html`
            <div class="space-y-2">
              <div class="text-sm font-medium">Popover title</div>
              <div class="text-xs text-muted-foreground">Popover body text.</div>
            </div>
          ` })}
        `,
      })}

      ${Tooltip({
        children: html`
          ${TooltipTrigger({ children: Button({ variant: "ghost", children: "Hover for tooltip" }) })}
          ${TooltipContent({ state: "open", children: "Tooltip content" })}
        `,
      })}

      ${HoverCard({
        children: html`
          ${HoverCardTrigger({ children: Button({ variant: "ghost", children: "Hover card" }) })}
          ${HoverCardContent({ state: "open", children: "Hover card content" })}
        `,
      })}

      ${Dialog({
        state: "open",
        children: html`
          ${DialogTrigger({ children: Button({ variant: "outline", children: "Open dialog" }) })}
          ${DialogContent({
            state: "open",
            children: html`
              ${DialogHeader({
                children: html`
                  ${DialogTitle({ children: "Dialog title" })}
                  ${DialogDescription({ children: "Dialog description" })}
                `,
              })}
              <div class="text-sm">Dialog body content.</div>
            `,
          })}
        `,
      })}

      ${Command({
        className: "max-w-md border",
        children: html`
          ${CommandInput({ placeholder: "Search commands" })}
          ${CommandList({
            children: html`
              ${CommandEmpty({ children: "No results" })}
              ${CommandGroup({
                children: html`
                  ${CommandItem({ children: "Open recent" })}
                  ${CommandItem({ children: "New file" })}
                  ${CommandSeparator({})}
                  ${CommandItem({ children: "Close" })}
                `,
              })}
            `,
          })}
        `,
      })}
    </div>
  `,
}

export const Extras = {
  render: () => {
    const runtimePreview =
      typeof document === "undefined" ? null : mountUiRuntime(document.createElement("div"))
    const classNames = cx("rounded-md", "border", "px-2", "py-1")

    return html`
      <div class="flex flex-col gap-6">
        <div class="space-y-2">
          <div class="text-xs uppercase text-muted-foreground">Avatar Group</div>
          ${AvatarGroup({
            children: html`
              ${Avatar({
                children: html`
                  ${AvatarImage({ src: "https://placehold.co/32x32" })}
                  ${AvatarBadge({ children: "✓" })}
                `,
              })}
              ${Avatar({ children: AvatarFallback({ children: "AP" }) })}
              ${AvatarGroupCount({ children: "+2" })}
            `,
          })}
        </div>

        <div class="space-y-2">
          <div class="text-xs uppercase text-muted-foreground">Command Dialog</div>
          ${CommandDialog({
            title: "Command Palette",
            description: "Search commands",
            children: html`
              ${CommandInput({ placeholder: "Search..." })}
              ${CommandList({
                children: html`
                  ${CommandGroup({
                    children: html`
                      ${CommandItem({ children: html`Open ${CommandShortcut({ children: "⌘O" })}` })}
                      ${CommandItem({ children: html`New ${CommandShortcut({ children: "⌘N" })}` })}
                    `,
                  })}
                `,
              })}
            `,
          })}
        </div>

        <div class="space-y-2">
          <div class="text-xs uppercase text-muted-foreground">Dialog Parts</div>
          ${Dialog({
            state: "open",
            children: html`
              ${DialogTrigger({ children: "Trigger" })}
              ${DialogPortal({
                children: html`
                  ${DialogOverlay({ state: "open" })}
                  ${DialogContent({
                    state: "open",
                    showCloseButton: false,
                    children: html`
                      ${DialogHeader({
                        children: html`
                          ${DialogTitle({ children: "Dialog Title" })}
                          ${DialogDescription({ children: "Dialog description" })}
                        `,
                      })}
                      <div class="text-sm">Dialog body content.</div>
                      ${DialogFooter({
                        showCloseButton: true,
                        children: html`${Button({ variant: "secondary", children: "Save" })}`,
                      })}
                      ${DialogClose({ children: "Close" })}
                    `,
                  })}
                `,
              })}
            `,
          })}
        </div>

        <div class="space-y-2">
          <div class="text-xs uppercase text-muted-foreground">Dropdown Menu Parts</div>
          ${DropdownMenu({
            children: html`
              ${DropdownMenuTrigger({ children: Button({ variant: "outline", children: "Open menu" }) })}
              ${DropdownMenuContent({
                state: "open",
                children: html`
                  ${DropdownMenuGroup({
                    children: html`
                      ${DropdownMenuCheckboxItem({ checked: true, children: "Auto run" })}
                      ${DropdownMenuRadioGroup({
                        children: html`
                          ${DropdownMenuRadioItem({ value: "fast", checked: true, children: "Fast" })}
                          ${DropdownMenuRadioItem({ value: "safe", children: "Safe" })}
                        `,
                      })}
                    `,
                  })}
                  ${DropdownMenuSeparator({})}
                  ${DropdownMenuSub({
                    children: html`
                      ${DropdownMenuSubTrigger({ children: "More" })}
                      ${DropdownMenuSubContent({
                        children: html`
                          ${DropdownMenuItem({
                            children: html`Advanced ${DropdownMenuShortcut({ children: "⌘A" })}`,
                          })}
                        `,
                      })}
                    `,
                  })}
                `,
              })}
            `,
          })}
          ${DropdownMenuPortal({ children: html`<div class="text-xs text-muted-foreground">Portal placeholder</div>` })}
        </div>

        <div class="space-y-2">
          <div class="text-xs uppercase text-muted-foreground">Popover Parts</div>
          ${Popover({
            children: html`
              ${PopoverTrigger({ children: Button({ variant: "outline", children: "Open popover" }) })}
              ${PopoverContent({
                state: "open",
                children: html`
                  ${PopoverHeader({
                    children: html`
                      ${PopoverTitle({ children: "Popover title" })}
                      ${PopoverDescription({ children: "Popover description" })}
                    `,
                  })}
                  ${PopoverAnchor({ children: "Anchor" })}
                `,
              })}
            `,
          })}
        </div>

        <div class="space-y-2">
          <div class="text-xs uppercase text-muted-foreground">Select Parts</div>
          ${Select({
            children: html`
              ${SelectTrigger({ children: SelectValue({ children: "Select mode" }) })}
              ${SelectContent({
                state: "open",
                children: html`
                  ${SelectScrollUpButton({})}
                  ${SelectGroup({
                    children: html`
                      ${SelectLabel({ children: "Modes" })}
                      ${SelectItem({ value: "fast", children: "Fast" })}
                      ${SelectItem({ value: "safe", children: "Safe" })}
                      ${SelectSeparator({})}
                      ${SelectItem({ value: "auto", children: "Auto" })}
                    `,
                  })}
                  ${SelectScrollDownButton({})}
                `,
              })}
            `,
          })}
        </div>

        <div class="space-y-2">
          <div class="text-xs uppercase text-muted-foreground">Tooltip Provider</div>
          ${TooltipProvider({
            children: Tooltip({
              children: html`
                ${TooltipTrigger({ children: Button({ variant: "ghost", children: "Hover me" }) })}
                ${TooltipContent({ state: "open", children: "Tooltip inside provider" })}
              `,
            }),
          })}
        </div>

        <div class="space-y-2">
          <div class="text-xs uppercase text-muted-foreground">Runtime Helpers</div>
          <div class="${classNames}">
            <div class="text-xs">cx() output: ${classNames}</div>
            <div class="text-xs">mountUiRuntime: ${runtimePreview ? "Effect created" : "No DOM"}</div>
          </div>
        </div>
      </div>
    `
  },
}
