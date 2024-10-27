import { FormEventHandler } from "react"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import {
  Card, CardContent, CardDescription, CardHeader, CardTitle
} from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { DashboardLayout } from "@/Layouts/DashboardLayout"
import { Head, useForm } from "@inertiajs/react"

export default function ForgotPassword({ status }: { status?: string }) {
  const { data, setData, post, processing, errors } = useForm({
    email: '',
  });

  const submit: FormEventHandler = (e) => {
    e.preventDefault();

    post(route('password.email'));
  };

  const InputError = ({ message }: { message: string | undefined }) => (
    message ? <Alert variant="destructive"><AlertDescription>{message}</AlertDescription></Alert> : null
  );

  return (
    <DashboardLayout>
      <Head title="Forgot Password" />

      <div className="w-full h-full justify-center items-center flex max-w-md mx-auto">
        <Card className="w-[400px]">
          <CardHeader>
            <CardTitle>Forgot Password</CardTitle>
            <CardDescription>Enter your email and we'll send you a password reset link.</CardDescription>
          </CardHeader>
          <CardContent>
            {status && <div className="mb-4 font-medium text-sm text-green-600">{status}</div>}

            <form onSubmit={submit}>
              <div className="mb-4">
                <Label htmlFor="email">Email</Label>
                <Input
                  id="email"
                  type="email"
                  name="email"
                  value={data.email}
                  className="mt-1 block w-full"
                  autoComplete="username"
                  autoFocus
                  onChange={(e) => setData('email', e.target.value)}
                />
                <InputError message={errors.email} />
              </div>

              <div className="flex items-center justify-end">
                <Button type="submit" variant="secondary" disabled={processing}>
                  Email Password Reset Link
                </Button>
              </div>
            </form>
          </CardContent>
        </Card>
      </div>
    </DashboardLayout>
  );
}