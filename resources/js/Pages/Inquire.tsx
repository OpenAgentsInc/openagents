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
import { CheckCircle2 } from "lucide-react"

interface Props {
  success?: string
}

export default function Inquire({ success }: Props) {
  const { data, setData, post, processing, errors } = useForm({
    inquiry_type: 'custom_agents',
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
              <CardTitle className="text-center">Thank You</CardTitle>
              <CardDescription className="text-center">Your inquiry has been received</CardDescription>
            </CardHeader>
            <CardContent>
              <p className="text-center text-zinc-600 dark:text-zinc-400">
                {success}
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
            <CardDescription>Send us your inquiry</CardDescription>
          </CardHeader>
          <CardContent>
            <form onSubmit={submit}>
              <div className="mb-4">
                <Label>Type of Inquiry</Label>
                <RadioGroup
                  defaultValue="custom_agents"
                  value={data.inquiry_type}
                  onValueChange={(value) => setData('inquiry_type', value)}
                  className="mt-2 space-y-2"
                >
                  <div className="flex items-center space-x-2">
                    <RadioGroupItem value="custom_agents" id="custom_agents" />
                    <Label htmlFor="custom_agents" className="cursor-pointer">Custom agents for business</Label>
                  </div>
                  <div className="flex items-center space-x-2">
                    <RadioGroupItem value="bulk_credits" id="bulk_credits" />
                    <Label htmlFor="bulk_credits" className="cursor-pointer">Purchasing developer credits in bulk</Label>
                  </div>
                  <div className="flex items-center space-x-2">
                    <RadioGroupItem value="other" id="other" />
                    <Label htmlFor="other" className="cursor-pointer">Other question</Label>
                  </div>
                </RadioGroup>
                <InputError message={errors.inquiry_type} />
              </div>

              <div className="mb-4">
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