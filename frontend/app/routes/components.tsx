import React from "react"
import {
  Accordion, AccordionContent, AccordionItem, AccordionTrigger
} from "../components/ui/accordion"
import { Alert, AlertDescription, AlertTitle } from "../components/ui/alert"
import { Avatar, AvatarFallback, AvatarImage } from "../components/ui/avatar"
import { Badge } from "../components/ui/badge"
import { Button } from "../components/ui/button"
import { Calendar } from "../components/ui/calendar"
import {
  Card, CardContent, CardDescription, CardHeader, CardTitle
} from "../components/ui/card"
import { Checkbox } from "../components/ui/checkbox"
import {
  Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle,
  DialogTrigger
} from "../components/ui/dialog"
import {
  DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuLabel,
  DropdownMenuSeparator, DropdownMenuTrigger
} from "../components/ui/dropdown-menu"
import {
  HoverCard, HoverCardContent, HoverCardTrigger
} from "../components/ui/hover-card"
import { Input } from "../components/ui/input"
import { Label } from "../components/ui/label"
import {
  NavigationMenu, NavigationMenuContent, NavigationMenuItem,
  NavigationMenuLink, NavigationMenuList, NavigationMenuTrigger
} from "../components/ui/navigation-menu"
import { Progress } from "../components/ui/progress"
import { RadioGroup, RadioGroupItem } from "../components/ui/radio-group"
import { ScrollArea } from "../components/ui/scroll-area"
import { Separator } from "../components/ui/separator"
import {
  Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle, SheetTrigger
} from "../components/ui/sheet"
import { Skeleton } from "../components/ui/skeleton"
import { Slider } from "../components/ui/slider"
import { Switch } from "../components/ui/switch"
import {
  Table, TableBody, TableCaption, TableCell, TableHead, TableHeader, TableRow
} from "../components/ui/table"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "../components/ui/tabs"
import { Textarea } from "../components/ui/textarea"
import { Toggle } from "../components/ui/toggle"
import {
  Tooltip, TooltipContent, TooltipProvider, TooltipTrigger
} from "../components/ui/tooltip"

export default function ComponentLibrary() {
  const [date, setDate] = React.useState<Date | undefined>(new Date())

  return (
    <ScrollArea className="h-full">
      <div className="container mx-auto py-8 px-4">
        <h1 className="text-3xl font-bold mb-8">Component Library</h1>

        <div className="grid gap-8">
          {/* Buttons Section */}
          <Card>
            <CardHeader>
              <CardTitle>Buttons</CardTitle>
              <CardDescription>Various button styles and variants</CardDescription>
            </CardHeader>
            <CardContent className="flex flex-wrap gap-4">
              <Button variant="default">Default</Button>
              <Button variant="secondary">Secondary</Button>
              <Button variant="destructive">Destructive</Button>
              <Button variant="outline">Outline</Button>
              <Button variant="ghost">Ghost</Button>
              <Button variant="link">Link</Button>
              <Button disabled>Disabled</Button>
              <Button size="sm">Small</Button>
              <Button size="lg">Large</Button>
            </CardContent>
          </Card>

          {/* Badges Section */}
          <Card>
            <CardHeader>
              <CardTitle>Badges</CardTitle>
              <CardDescription>Status indicators and labels</CardDescription>
            </CardHeader>
            <CardContent className="flex flex-wrap gap-4">
              <Badge>Default</Badge>
              <Badge variant="secondary">Secondary</Badge>
              <Badge variant="destructive">Destructive</Badge>
              <Badge variant="outline">Outline</Badge>
            </CardContent>
          </Card>

          {/* Form Controls Section */}
          <Card>
            <CardHeader>
              <CardTitle>Form Controls</CardTitle>
              <CardDescription>Input elements and form components</CardDescription>
            </CardHeader>
            <CardContent className="grid gap-6">
              <div className="grid gap-2">
                <Label htmlFor="email">Email</Label>
                <Input id="email" placeholder="Enter your email" type="email" />
              </div>
              <div className="flex items-center gap-2">
                <Switch id="airplane-mode" />
                <Label htmlFor="airplane-mode">Airplane Mode</Label>
              </div>
            </CardContent>
          </Card>

          {/* Tabs Section */}
          <Card>
            <CardHeader>
              <CardTitle>Tabs</CardTitle>
              <CardDescription>Switch between different views</CardDescription>
            </CardHeader>
            <CardContent>
              <Tabs defaultValue="account">
                <TabsList>
                  <TabsTrigger value="account">Account</TabsTrigger>
                  <TabsTrigger value="password">Password</TabsTrigger>
                  <TabsTrigger value="settings">Settings</TabsTrigger>
                </TabsList>
                <TabsContent value="account">Account settings here</TabsContent>
                <TabsContent value="password">Password settings here</TabsContent>
                <TabsContent value="settings">Other settings here</TabsContent>
              </Tabs>
            </CardContent>
          </Card>

          {/* Accordion Section */}
          <Card>
            <CardHeader>
              <CardTitle>Accordion</CardTitle>
              <CardDescription>Expandable content sections</CardDescription>
            </CardHeader>
            <CardContent>
              <Accordion type="single" collapsible>
                <AccordionItem value="item-1">
                  <AccordionTrigger>Is it accessible?</AccordionTrigger>
                  <AccordionContent>
                    Yes. It adheres to the WAI-ARIA design pattern.
                  </AccordionContent>
                </AccordionItem>
                <AccordionItem value="item-2">
                  <AccordionTrigger>Is it styled?</AccordionTrigger>
                  <AccordionContent>
                    Yes. It comes with default styles that matches your theme.
                  </AccordionContent>
                </AccordionItem>
              </Accordion>
            </CardContent>
          </Card>

          {/* Alert Section */}
          <Card>
            <CardHeader>
              <CardTitle>Alerts</CardTitle>
              <CardDescription>Informative message boxes</CardDescription>
            </CardHeader>
            <CardContent className="grid gap-4">
              <Alert>
                <AlertTitle>Heads up!</AlertTitle>
                <AlertDescription>
                  You can add components to your app using the cli.
                </AlertDescription>
              </Alert>
              <Alert variant="destructive">
                <AlertTitle>Error</AlertTitle>
                <AlertDescription>
                  Something went wrong! Please try again.
                </AlertDescription>
              </Alert>
            </CardContent>
          </Card>

          {/* Avatar Section */}
          <Card>
            <CardHeader>
              <CardTitle>Avatars</CardTitle>
              <CardDescription>User profile pictures and fallbacks</CardDescription>
            </CardHeader>
            <CardContent className="flex gap-4">
              <Avatar>
                <AvatarImage src="https://github.com/shadcn.png" />
                <AvatarFallback>CN</AvatarFallback>
              </Avatar>
              <Avatar>
                <AvatarImage src="https://github.com/shadcn-wrong.png" />
                <AvatarFallback>JD</AvatarFallback>
              </Avatar>
            </CardContent>
          </Card>

          {/* Dropdown Menu Section */}
          <Card>
            <CardHeader>
              <CardTitle>Dropdown Menu</CardTitle>
              <CardDescription>Contextual menus and actions</CardDescription>
            </CardHeader>
            <CardContent>
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button variant="outline">Open Menu</Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent>
                  <DropdownMenuLabel>My Account</DropdownMenuLabel>
                  <DropdownMenuSeparator />
                  <DropdownMenuItem>Profile</DropdownMenuItem>
                  <DropdownMenuItem>Billing</DropdownMenuItem>
                  <DropdownMenuItem>Team</DropdownMenuItem>
                  <DropdownMenuItem>Subscription</DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            </CardContent>
          </Card>

          {/* Dialog Section */}
          <Card>
            <CardHeader>
              <CardTitle>Dialog</CardTitle>
              <CardDescription>Modal dialogs and popovers</CardDescription>
            </CardHeader>
            <CardContent>
              <Dialog>
                <DialogTrigger asChild>
                  <Button variant="outline">Open Dialog</Button>
                </DialogTrigger>
                <DialogContent>
                  <DialogHeader>
                    <DialogTitle>Are you sure?</DialogTitle>
                    <DialogDescription>
                      This action cannot be undone. This will permanently delete your account
                      and remove your data from our servers.
                    </DialogDescription>
                  </DialogHeader>
                  <div className="flex justify-end gap-3 mt-4">
                    <Button variant="outline">Cancel</Button>
                    <Button variant="destructive">Delete Account</Button>
                  </div>
                </DialogContent>
              </Dialog>
            </CardContent>
          </Card>

          {/* Calendar Section */}
          <Card>
            <CardHeader>
              <CardTitle>Calendar</CardTitle>
              <CardDescription>Date picker component</CardDescription>
            </CardHeader>
            <CardContent>
              <Calendar
                mode="single"
                selected={date}
                onSelect={setDate}
                className="rounded-md border"
              />
            </CardContent>
          </Card>

          {/* Checkbox and Radio Section */}
          <Card>
            <CardHeader>
              <CardTitle>Selection Controls</CardTitle>
              <CardDescription>Checkboxes and radio buttons</CardDescription>
            </CardHeader>
            <CardContent className="grid gap-6">
              <div className="flex items-center space-x-2">
                <Checkbox id="terms" />
                <Label htmlFor="terms">Accept terms and conditions</Label>
              </div>
              <RadioGroup defaultValue="option-one">
                <div className="flex items-center space-x-2">
                  <RadioGroupItem value="option-one" id="option-one" />
                  <Label htmlFor="option-one">Option One</Label>
                </div>
                <div className="flex items-center space-x-2">
                  <RadioGroupItem value="option-two" id="option-two" />
                  <Label htmlFor="option-two">Option Two</Label>
                </div>
              </RadioGroup>
            </CardContent>
          </Card>

          {/* Table Section */}
          <Card>
            <CardHeader>
              <CardTitle>Table</CardTitle>
              <CardDescription>Data tables with sorting and selection</CardDescription>
            </CardHeader>
            <CardContent>
              <Table>
                <TableCaption>A list of recent invoices</TableCaption>
                <TableHeader>
                  <TableRow>
                    <TableHead>Invoice</TableHead>
                    <TableHead>Status</TableHead>
                    <TableHead>Method</TableHead>
                    <TableHead className="text-right">Amount</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  <TableRow>
                    <TableCell>INV001</TableCell>
                    <TableCell>Paid</TableCell>
                    <TableCell>Credit Card</TableCell>
                    <TableCell className="text-right">$250.00</TableCell>
                  </TableRow>
                  <TableRow>
                    <TableCell>INV002</TableCell>
                    <TableCell>Pending</TableCell>
                    <TableCell>PayPal</TableCell>
                    <TableCell className="text-right">$150.00</TableCell>
                  </TableRow>
                </TableBody>
              </Table>
            </CardContent>
          </Card>

          {/* Progress and Loading Section */}
          <Card>
            <CardHeader>
              <CardTitle>Progress and Loading</CardTitle>
              <CardDescription>Loading states and progress indicators</CardDescription>
            </CardHeader>
            <CardContent className="grid gap-6">
              <Progress value={60} className="w-full" />
              <div className="grid grid-cols-3 gap-4">
                <Skeleton className="h-4 w-full" />
                <Skeleton className="h-4 w-full" />
                <Skeleton className="h-4 w-full" />
              </div>
            </CardContent>
          </Card>

          {/* Toggle Section */}
          <Card>
            <CardHeader>
              <CardTitle>Toggle</CardTitle>
              <CardDescription>Toggle buttons and switches</CardDescription>
            </CardHeader>
            <CardContent className="flex gap-4">
              <Toggle>Bold</Toggle>
              <Toggle>Italic</Toggle>
              <Toggle>Underline</Toggle>
            </CardContent>
          </Card>

          {/* Navigation Menu Section */}
          <Card>
            <CardHeader>
              <CardTitle>Navigation Menu</CardTitle>
              <CardDescription>Advanced navigation components</CardDescription>
            </CardHeader>
            <CardContent>
              <NavigationMenu>
                <NavigationMenuList>
                  <NavigationMenuItem>
                    <NavigationMenuTrigger>Getting Started</NavigationMenuTrigger>
                    <NavigationMenuContent>
                      <div className="grid gap-3 p-4 w-[400px]">
                        <NavigationMenuLink>Introduction</NavigationMenuLink>
                        <NavigationMenuLink>Installation</NavigationMenuLink>
                        <NavigationMenuLink>Typography</NavigationMenuLink>
                      </div>
                    </NavigationMenuContent>
                  </NavigationMenuItem>
                  <NavigationMenuItem>
                    <NavigationMenuTrigger>Components</NavigationMenuTrigger>
                    <NavigationMenuContent>
                      <div className="grid gap-3 p-4 w-[400px]">
                        <NavigationMenuLink>Button</NavigationMenuLink>
                        <NavigationMenuLink>Dropdown Menu</NavigationMenuLink>
                        <NavigationMenuLink>Table</NavigationMenuLink>
                      </div>
                    </NavigationMenuContent>
                  </NavigationMenuItem>
                </NavigationMenuList>
              </NavigationMenu>
            </CardContent>
          </Card>

          {/* Hover Card Section */}
          <Card>
            <CardHeader>
              <CardTitle>Hover Card</CardTitle>
              <CardDescription>Popup cards on hover</CardDescription>
            </CardHeader>
            <CardContent>
              <HoverCard>
                <HoverCardTrigger asChild>
                  <Button variant="link">@shadcn</Button>
                </HoverCardTrigger>
                <HoverCardContent>
                  <div className="flex gap-4">
                    <Avatar>
                      <AvatarImage src="https://github.com/shadcn.png" />
                      <AvatarFallback>CN</AvatarFallback>
                    </Avatar>
                    <div>
                      <h4 className="text-sm font-semibold">@shadcn</h4>
                      <p className="text-sm">
                        Developer and creator of shadcn/ui.
                      </p>
                    </div>
                  </div>
                </HoverCardContent>
              </HoverCard>
            </CardContent>
          </Card>

          {/* Tooltip Section */}
          <Card>
            <CardHeader>
              <CardTitle>Tooltip</CardTitle>
              <CardDescription>Informative tooltips on hover</CardDescription>
            </CardHeader>
            <CardContent className="flex gap-4">
              <TooltipProvider>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button variant="outline">Hover Me</Button>
                  </TooltipTrigger>
                  <TooltipContent>
                    <p>Add to calendar</p>
                  </TooltipContent>
                </Tooltip>
              </TooltipProvider>
            </CardContent>
          </Card>

          {/* Sheet Section */}
          <Card>
            <CardHeader>
              <CardTitle>Sheet</CardTitle>
              <CardDescription>Slide-out panels from any edge</CardDescription>
            </CardHeader>
            <CardContent className="flex gap-4">
              <Sheet>
                <SheetTrigger asChild>
                  <Button variant="outline">Open Sheet</Button>
                </SheetTrigger>
                <SheetContent>
                  <SheetHeader>
                    <SheetTitle>Edit profile</SheetTitle>
                    <SheetDescription>
                      Make changes to your profile here. Click save when you're done.
                    </SheetDescription>
                  </SheetHeader>
                  <div className="grid gap-4 py-4">
                    <div className="grid gap-2">
                      <Label htmlFor="name">Name</Label>
                      <Input id="name" placeholder="Enter your name" />
                    </div>
                    <div className="grid gap-2">
                      <Label htmlFor="bio">Bio</Label>
                      <Textarea id="bio" placeholder="Tell us about yourself" />
                    </div>
                  </div>
                  <div className="flex justify-end">
                    <Button>Save changes</Button>
                  </div>
                </SheetContent>
              </Sheet>
            </CardContent>
          </Card>

          {/* Slider Section */}
          <Card>
            <CardHeader>
              <CardTitle>Slider</CardTitle>
              <CardDescription>Range and slider inputs</CardDescription>
            </CardHeader>
            <CardContent className="py-6">
              <Slider
                defaultValue={[33]}
                max={100}
                step={1}
                className="w-full"
              />
            </CardContent>
          </Card>

          {/* Text Input Section */}
          <Card>
            <CardHeader>
              <CardTitle>Text Inputs</CardTitle>
              <CardDescription>Various text input components</CardDescription>
            </CardHeader>
            <CardContent className="grid gap-4">
              <div className="grid gap-2">
                <Label htmlFor="message">Message</Label>
                <Textarea
                  id="message"
                  placeholder="Type your message here"
                  className="min-h-[100px]"
                />
              </div>
            </CardContent>
          </Card>
        </div>

        <div className="h-8" />
        <Separator className="my-4" />
        <p className="text-sm text-muted-foreground text-center pb-8">
          All core shadcn/ui components have been showcased
        </p>
      </div>
    </ScrollArea>
  )
}
