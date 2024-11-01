import { PageProps } from "@/types"
import { Head } from "@inertiajs/react"
import MainLayout from "@/Layouts/MainLayout"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"

interface Project {
  id: number
  name: string
  description: string
  user_id: number | null
  team_id: number | null
  status: string
  team?: {
    name: string
  }
  files: any[]
}

interface Props extends PageProps {
  project: Project
}

export default function Show({ project }: Props) {
  return (
    <MainLayout>
      <Head title={project.name} />
      <div className="p-6 space-y-6">
        {/* Project Header */}
        <div className="space-y-4">
          <div className="flex items-center justify-between">
            <h1 className="text-3xl font-bold">{project.name}</h1>
            <div className="flex items-center space-x-2">
              <Badge variant="outline">
                {project.team_id ? "Team" : "Personal"}
              </Badge>
              <Badge>Private</Badge>
            </div>
          </div>
          <p className="text-muted-foreground">{project.description}</p>
        </div>

        {/* Project Details */}
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          <Card>
            <CardHeader>
              <CardTitle className="text-sm font-medium">Project Type</CardTitle>
            </CardHeader>
            <CardContent>
              <p className="text-lg font-semibold">
                {project.team_id
                  ? `Team Project (${project.team?.name})`
                  : "Personal Project"}
              </p>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className="text-sm font-medium">
                Custom Instructions
              </CardTitle>
            </CardHeader>
            <CardContent>
              <p className="text-muted-foreground">No custom instructions set</p>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className="text-sm font-medium">Knowledge Files</CardTitle>
            </CardHeader>
            <CardContent>
              <p className="text-muted-foreground">No knowledge files uploaded</p>
            </CardContent>
          </Card>
        </div>

        {/* Project Content */}
        <Tabs defaultValue="overview" className="space-y-4">
          <TabsList>
            <TabsTrigger value="overview">Overview</TabsTrigger>
            <TabsTrigger value="files">Files</TabsTrigger>
            <TabsTrigger value="settings">Settings</TabsTrigger>
          </TabsList>

          <TabsContent value="overview">
            <Card>
              <CardContent className="p-6">
                <div className="space-y-4">
                  <div>
                    <h3 className="text-lg font-semibold mb-2">Description</h3>
                    <p className="text-muted-foreground">{project.description}</p>
                  </div>
                  
                  <div>
                    <h3 className="text-lg font-semibold mb-2">Status</h3>
                    <Badge variant="secondary">{project.status}</Badge>
                  </div>
                </div>
              </CardContent>
            </Card>
          </TabsContent>

          <TabsContent value="files">
            <Card>
              <CardContent className="p-6">
                {project.files.length > 0 ? (
                  <div className="space-y-4">
                    {project.files.map((file) => (
                      <div key={file.id} className="flex items-center justify-between">
                        <span>{file.name}</span>
                        <Badge>{file.type}</Badge>
                      </div>
                    ))}
                  </div>
                ) : (
                  <p className="text-muted-foreground">No files uploaded yet</p>
                )}
              </CardContent>
            </Card>
          </TabsContent>

          <TabsContent value="settings">
            <Card>
              <CardContent className="p-6">
                <p className="text-muted-foreground">Project settings coming soon...</p>
              </CardContent>
            </Card>
          </TabsContent>
        </Tabs>
      </div>
    </MainLayout>
  )
}