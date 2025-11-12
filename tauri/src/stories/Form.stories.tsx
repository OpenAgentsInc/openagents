import type { Meta, StoryObj } from '@storybook/react-vite';
import { useForm } from 'react-hook-form';
import { z } from 'zod';
import { zodResolver } from '@hookform/resolvers/zod';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import {
  Form,
  FormField,
  FormItem,
  FormLabel,
  FormControl,
  FormDescription,
  FormMessage,
} from '@/components/ui/form';

const schema = z.object({
  name: z.string().min(1, 'Name is required'),
  email: z.string().email('Enter a valid email'),
});

type Values = z.infer<typeof schema>;

const meta = {
  title: 'UI/Form',
  component: Form,
  argTypes: {},
  args: {},
} satisfies Meta<typeof Form>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: () => {
    const form = useForm<Values>({
      resolver: zodResolver(schema),
      defaultValues: { name: '', email: '' },
      mode: 'onBlur',
    });
    const onSubmit = (values: Values) => {
      // For demonstration purposes
      alert(`Submitted: ${JSON.stringify(values, null, 2)}`);
    };
    return (
      <Form {...form}>
        <form onSubmit={form.handleSubmit(onSubmit)} style={{ display: 'grid', gap: 12, width: 360 }}>
          <FormField
            control={form.control}
            name="name"
            render={({ field }) => (
              <FormItem>
                <FormLabel>Name</FormLabel>
                <FormControl>
                  <Input placeholder="Ada Lovelace" {...field} />
                </FormControl>
                <FormDescription>Your full name.</FormDescription>
                <FormMessage />
              </FormItem>
            )}
          />

          <FormField
            control={form.control}
            name="email"
            render={({ field }) => (
              <FormItem>
                <FormLabel>Email</FormLabel>
                <FormControl>
                  <Input type="email" placeholder="ada@example.com" {...field} />
                </FormControl>
                <FormDescription>We will never share your email.</FormDescription>
                <FormMessage />
              </FormItem>
            )}
          />

          <Button type="submit">Submit</Button>
        </form>
      </Form>
    );
  },
};

