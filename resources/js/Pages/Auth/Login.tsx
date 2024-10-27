import { FormEventHandler } from "react"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import {
  Card, CardContent, CardDescription, CardHeader, CardTitle
} from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { BlankLayout } from "@/Layouts/BlankLayout"
import { Head, Link, useForm } from "@inertiajs/react"

export default function Login({ status, canResetPassword }: { status?: string, canResetPassword: boolean }) {
  const { data, setData, post, processing, errors, reset } = useForm({
    email: '',
    password: '',
    remember: true,
  });

  const submit: FormEventHandler = (e) => {
    e.preventDefault();

    post(route('login'), {
      onFinish: () => reset('password'),
    });
  };

  const InputError = ({ message }: { message: string | undefined }) => (
    message ? <Alert variant="destructive"><AlertDescription>{message}</AlertDescription></Alert> : null
  );

  return (
    <BlankLayout>
      <Head title="Log in" />

      <div className="w-full h-full justify-center items-center flex max-w-md mx-auto">
        <Card className="w-[400px]">
          <CardHeader>
            <CardTitle>Log in</CardTitle>
            <CardDescription>Access your client portal</CardDescription>
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

              <div className="mb-4">
                <Label htmlFor="password">Password</Label>
                <Input
                  id="password"
                  type="password"
                  name="password"
                  value={data.password}
                  className="mt-1 block w-full"
                  autoComplete="current-password"
                  onChange={(e) => setData('password', e.target.value)}
                />
                <InputError message={errors.password} />
              </div>

              <div className="flex items-center justify-between">
                {canResetPassword && (
                  <Link
                    href={route('password.request')}
                    className="text-sm text-zinc-600 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100"
                  >
                    Forgot your password?
                  </Link>
                )}

                <Button type="submit" variant="secondary" disabled={processing}>
                  Log in
                </Button>
              </div>
            </form>
          </CardContent>
        </Card>
      </div>
    </BlankLayout>
  );
}