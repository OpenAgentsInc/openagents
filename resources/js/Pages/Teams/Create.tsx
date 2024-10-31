import { FormEventHandler } from "react"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import {
  Card, CardContent, CardDescription, CardHeader, CardTitle
} from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { BlankLayout } from "@/Layouts/BlankLayout"
import MainLayout from "@/Layouts/MainLayout"
import { Head, useForm } from "@inertiajs/react"

export default function CreateTeam() {
  const { data, setData, post, processing, errors } = useForm({
    name: '',
  });

  const submit: FormEventHandler = (e) => {
    e.preventDefault();
    post(route('teams.store'));
  };

  const InputError = ({ message }: { message: string | undefined }) => (
    message ? <Alert variant="destructive"><AlertDescription>{message}</AlertDescription></Alert> : null
  );

  return (
    <MainLayout>
      <Head title="Create Team" />

      <div className="w-full h-full justify-center items-center flex max-w-md mx-auto">
        <Card className="w-[400px]">
          <CardHeader>
            <CardTitle>Create Team</CardTitle>
            <CardDescription>Create a new team to collaborate with others</CardDescription>
          </CardHeader>
          <CardContent>
            <form onSubmit={submit}>
              <div className="mb-4">
                <Label htmlFor="name">Team Name</Label>
                <Input
                  id="name"
                  name="name"
                  value={data.name}
                  className="mt-1 block w-full"
                  autoFocus
                  onChange={(e) => setData('name', e.target.value)}
                  required
                />
                <InputError message={errors.name} />
              </div>

              <div className="flex items-center justify-end">
                <Button variant="secondary" type="submit" disabled={processing}>
                  Create Team
                </Button>
              </div>
            </form>
          </CardContent>
        </Card>
      </div>
    </MainLayout>
  );
}
