import { PlusCircle, Search } from "lucide-react"
import MainLayout from "@/Layouts/MainLayout"
import { PageProps } from "@/types"
import { Head } from "@inertiajs/react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { Badge } from "@/components/ui/badge"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"

// Mock data - replace with real data from your backend
const leads = [
  {
    id: 1,
    name: "John Smith",
    company: "Tech Corp",
    email: "john@techcorp.com",
    phone: "(555) 123-4567",
    status: "New",
    value: "$50,000",
    lastContact: "2024-01-15",
  },
  {
    id: 2,
    name: "Sarah Johnson",
    company: "Marketing Pro",
    email: "sarah@marketingpro.com",
    phone: "(555) 987-6543",
    status: "In Progress",
    value: "$25,000",
    lastContact: "2024-01-14",
  },
  {
    id: 3,
    name: "Mike Williams",
    company: "Sales Solutions",
    email: "mike@salessolutions.com",
    phone: "(555) 456-7890",
    status: "Qualified",
    value: "$75,000",
    lastContact: "2024-01-13",
  },
]

const stats = [
  { label: "Total Leads", value: "145" },
  { label: "Open Deals", value: "12" },
  { label: "Closed (This Month)", value: "8" },
  { label: "Pipeline Value", value: "$1.2M" },
]

export default function CRM({ auth }: PageProps) {
  return (
    <MainLayout>
      <Head title="CRM" />
      <div className="p-6 space-y-6">
        {/* Stats Cards */}
        <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
          {stats.map((stat, index) => (
            <Card key={index}>
              <CardHeader className="py-4">
                <CardTitle className="text-sm font-medium">{stat.label}</CardTitle>
              </CardHeader>
              <CardContent>
                <p className="text-2xl font-bold">{stat.value}</p>
              </CardContent>
            </Card>
          ))}
        </div>

        {/* Main Content */}
        <Tabs defaultValue="leads" className="space-y-4">
          <div className="flex items-center justify-between">
            <TabsList>
              <TabsTrigger value="leads">Leads</TabsTrigger>
              <TabsTrigger value="contacts">Contacts</TabsTrigger>
              <TabsTrigger value="deals">Deals</TabsTrigger>
            </TabsList>
            <div className="flex items-center space-x-2">
              <div className="relative">
                <Search className="absolute left-2 top-2.5 h-4 w-4 text-muted-foreground" />
                <Input placeholder="Search leads..." className="pl-8" />
              </div>
              <Button>
                <PlusCircle className="mr-2 h-4 w-4" />
                Add Lead
              </Button>
            </div>
          </div>

          <TabsContent value="leads" className="space-y-4">
            <Card>
              <CardContent className="p-0">
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Name</TableHead>
                      <TableHead>Company</TableHead>
                      <TableHead>Email</TableHead>
                      <TableHead>Phone</TableHead>
                      <TableHead>Status</TableHead>
                      <TableHead>Value</TableHead>
                      <TableHead>Last Contact</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {leads.map((lead) => (
                      <TableRow key={lead.id}>
                        <TableCell className="font-medium">{lead.name}</TableCell>
                        <TableCell>{lead.company}</TableCell>
                        <TableCell>{lead.email}</TableCell>
                        <TableCell>{lead.phone}</TableCell>
                        <TableCell>
                          <Badge
                            variant={
                              lead.status === "New"
                                ? "default"
                                : lead.status === "In Progress"
                                ? "secondary"
                                : "success"
                            }
                          >
                            {lead.status}
                          </Badge>
                        </TableCell>
                        <TableCell>{lead.value}</TableCell>
                        <TableCell>{lead.lastContact}</TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </CardContent>
            </Card>
          </TabsContent>

          <TabsContent value="contacts">
            <Card>
              <CardContent className="p-4">
                <p className="text-sm text-muted-foreground">
                  Contacts view coming soon...
                </p>
              </CardContent>
            </Card>
          </TabsContent>

          <TabsContent value="deals">
            <Card>
              <CardContent className="p-4">
                <p className="text-sm text-muted-foreground">
                  Deals view coming soon...
                </p>
              </CardContent>
            </Card>
          </TabsContent>
        </Tabs>
      </div>
    </MainLayout>
  )
}