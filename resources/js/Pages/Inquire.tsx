import { CheckCircle2 } from "lucide-react"
import { FormEventHandler } from "react"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import {
  Card, CardContent, CardDescription, CardHeader, CardTitle
} from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group"
import { Textarea } from "@/components/ui/textarea"
import { DashboardLayout } from "@/Layouts/DashboardLayout"
import { Head, useForm } from "@inertiajs/react"

interface Props {
  success?: string
}

export default function Inquire({ success }: Props) {
  const { data, setData, post, processing, errors } = useForm({
    inquiry_type: 'request_demo',
    email: '',
    comment: '',
  });

  const submit: FormEventHandler = (e) => {
    e.preventDefault();

    post(route('inquire.submit'));
  };

  const InputError = ({ message }: { message: string | undefined }) => (
    message ? <Alert variant="destructive"><AlertDescription>{message}</AlertDescription></Alert> : null
  );

  if (success) {
    return (
      <DashboardLayout>
        <Head title="Thank You" />

        <div className="w-full h-full justify-center items-center flex max-w-md mx-auto">
          <Card className="w-[400px]">
            <CardHeader>
              <div className="flex justify-center mb-4">
                <CheckCircle2 className="h-12 w-12 text-green-500" />
              </div>
              <CardTitle className="text-center">Thank you!</CardTitle>
              <CardDescription className="text-center">Inquiry received</CardDescription>
            </CardHeader>
            <CardContent>
              <p className="text-center text-zinc-600 dark:text-zinc-400">
                We'll follow up soon. Or feel free to <a href="https://calendly.com/christopher-david-openagents/30min" target="_blank" className="hover:underline text-white">book a call with our founder now</a>.
              </p>
              <div className="mt-6 flex justify-center">
                <Button
                  variant="secondary"
                  onClick={() => window.location.href = route('home')}
                >
                  Return Home
                </Button>
              </div>
            </CardContent>
          </Card>
        </div>
      </DashboardLayout>
    )
  }

  return (
    <DashboardLayout>
      <Head title="Inquire" />

      <div className="w-full h-full justify-center items-center flex max-w-md mx-auto">
        <Card className="w-[400px]">
          <CardHeader>
            <CardTitle>Inquire</CardTitle>
            <CardDescription>How can we help?</CardDescription>
          </CardHeader>
          <CardContent>
            <form onSubmit={submit}>
              <div className="mb-4">
                {/* <Label>Type of Inquiry</Label> */}
                <RadioGroup
                  defaultValue="general_question"
                  value={data.inquiry_type}
                  onValueChange={(value) => setData('inquiry_type', value)}
                  className="mt-2 space-y-2"
                >
                  <div className="flex items-center space-x-2">
                    <RadioGroupItem value="request_demo" id="request_demo" />
                    <Label htmlFor="request_demo" className="cursor-pointer">Request a demo</Label>
                  </div>
                  <div className="flex items-center space-x-2">
                    <RadioGroupItem value="custom_agents" id="custom_agents" />
                    <Label htmlFor="custom_agents" className="cursor-pointer">Interested in custom AI agents</Label>
                  </div>
                  <div className="flex items-center space-x-2">
                    <RadioGroupItem value="bulk_credits" id="bulk_credits" />
                    <Label htmlFor="bulk_credits" className="cursor-pointer">Bulk purchase AutoDev agent credit</Label>
                  </div>
                  <div className="flex items-center space-x-2">
                    <RadioGroupItem value="general_question" id="general_question" />
                    <Label htmlFor="general_question" className="cursor-pointer">General question</Label>
                  </div>
                  <div className="flex items-center space-x-2">
                    <RadioGroupItem value="other" id="other" />
                    <Label htmlFor="other" className="cursor-pointer">Other</Label>
                  </div>
                </RadioGroup>
                <InputError message={errors.inquiry_type} />
              </div>

              <div className="my-8">
                <Label htmlFor="email">Email</Label>
                <Input
                  id="email"
                  type="email"
                  name="email"
                  value={data.email}
                  className="mt-1 block w-full"
                  autoComplete="email"
                  onChange={(e) => setData('email', e.target.value)}
                  required
                />
                <InputError message={errors.email} />
              </div>

              <div className="mb-4">
                <Label htmlFor="comment">Comment</Label>
                <Textarea
                  id="comment"
                  name="comment"
                  value={data.comment}
                  className="mt-1 block w-full"
                  rows={4}
                  onChange={(e) => setData('comment', e.target.value)}
                  required
                />
                <InputError message={errors.comment} />
              </div>

              <div className="flex items-center justify-end">
                <Button type="submit" variant="secondary" disabled={processing}>
                  Submit Inquiry
                </Button>
              </div>
            </form>
          </CardContent>
        </Card>
      </div>
    </DashboardLayout>
  );
}
