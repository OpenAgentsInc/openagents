import type { Meta, StoryObj } from '@storybook/react-vite';
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs';

const meta = {
  title: 'UI/Tabs',
  component: Tabs,
  argTypes: {
    defaultValue: { control: 'select', options: ['account', 'password', 'billing'] },
    width: { control: 'number' },
  },
  args: {
    defaultValue: 'account',
    width: 480,
  },
} satisfies Meta<typeof Tabs>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: ({ defaultValue, width }) => (
    <Tabs defaultValue={defaultValue as string} style={{ width: Number(width) }}>
      <TabsList>
        <TabsTrigger value="account">Account</TabsTrigger>
        <TabsTrigger value="password">Password</TabsTrigger>
        <TabsTrigger value="billing">Billing</TabsTrigger>
      </TabsList>
      <div style={{ padding: 12, border: '1px solid var(--border)', borderRadius: 6 }}>
        <TabsContent value="account">
          Manage your account settings here.
        </TabsContent>
        <TabsContent value="password">
          Change your password securely.
        </TabsContent>
        <TabsContent value="billing">
          Update your billing information and view invoices.
        </TabsContent>
      </div>
    </Tabs>
  ),
};

