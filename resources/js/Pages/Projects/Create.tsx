import { FormEventHandler } from "react"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import {
  Card, CardContent, CardDescription, CardHeader, CardTitle
} from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Textarea } from "@/components/ui/textarea"
import MainLayout from "@/Layouts/MainLayout"
import { Head, useForm } from "@inertiajs/react"

interface Props {
  teamName?: string;
}

export default function CreateProject({ teamName }: Props) {
  const { data, setData, post, processing, errors } = useForm({
    name: '',
    description: '',
  });

  const submit: FormEventHandler = (e) => {
    e.preventDefault();
    post(route('projects.store'));
  };

  const InputError = ({ message }: { message: string | undefined }) => (
    message ? <Alert variant="destructive"><AlertDescription>{message}</AlertDescription></Alert> : null
  );

  const title = teamName ? `Create a ${teamName} project` : "Create a personal project";

  return (
    <MainLayout>
      <Head title="Create Project" />

      <div className="w-full h-full justify-center items-center flex max-w-md mx-auto">
        <Card className="w-[400px]">
          <CardHeader>
            <CardTitle>{title}</CardTitle>
            <CardDescription>Create a new project to organize your work</CardDescription>
          </CardHeader>
          <CardContent>
            <form onSubmit={submit}>
              <div className="mb-4">
                <Label htmlFor="name">What are you working on?</Label>
                <Input
                  id="name"
                  name="name"
                  value={data.name}
                  className="mt-1 block w-full"
                  autoFocus
                  placeholder="Name your project"
                  onChange={(e) => setData('name', e.target.value)}
                  required
                />
                <InputError message={errors.name} />
              </div>

              <div className="mb-4">
                <Label htmlFor="description">What are you trying to achieve?</Label>
                <Textarea
                  id="description"
                  name="description"
                  value={data.description}
                  className="mt-1 block w-full"
                  rows={3}
                  placeholder="Describe your project, goals, subject, etc..."
                  onChange={(e) => setData('description', e.target.value)}
                  required
                />
                <InputError message={errors.description} />
              </div>

              <div className="flex items-center justify-end">
                <Button variant="secondary" type="submit" disabled={processing}>
                  Create Project
                </Button>
              </div>
            </form>
          </CardContent>
        </Card>
      </div>
    </MainLayout>
  );
}