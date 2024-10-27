import { FormEventHandler } from "react"
import { Button } from "@/components/ui/button"
import {
  Card, CardContent, CardDescription, CardHeader, CardTitle
} from "@/components/ui/card"
import { UnauthedLayout } from "@/Layouts/UnauthedLayout"
import { Head, Link, useForm } from "@inertiajs/react"

export default function VerifyEmail({ status }: { status?: string }) {
  const { post, processing } = useForm({});

  const submit: FormEventHandler = (e) => {
    e.preventDefault();

    post(route('verification.send'));
  };

  return (
    <UnauthedLayout>
      <Head title="Email Verification" />

      <div className="w-full h-full justify-center items-center flex max-w-md mx-auto">
        <Card className="w-[400px]">
          <CardHeader>
            <CardTitle>Email Verification</CardTitle>
            <CardDescription>Verify your email to get started</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="mb-4 text-sm text-zinc-600 dark:text-zinc-400">
              Thanks for signing up! Before getting started, could you verify
              your email address by clicking on the link we just emailed to
              you? If you didn't receive the email, we will gladly send you
              another.
            </div>

            {status === 'verification-link-sent' && (
              <div className="mb-4 text-sm font-medium text-green-600">
                A new verification link has been sent to the email address
                you provided during registration.
              </div>
            )}

            <form onSubmit={submit}>
              <div className="flex items-center justify-between">
                <Button type="submit" variant="secondary" disabled={processing}>
                  Resend Verification Email
                </Button>

                <Link
                  href={route('logout')}
                  method="post"
                  as="button"
                  className="text-sm text-zinc-600 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100"
                >
                  Log Out
                </Link>
              </div>
            </form>
          </CardContent>
        </Card>
      </div>
    </UnauthedLayout>
  );
}
