import type { Meta, StoryObj } from '@storybook/react-vite';
import { Toaster } from '@/components/ui/sonner';
import { toast } from 'sonner';
import { Button } from '@/components/ui/button';

const meta = {
  title: 'UI/Sonner',
  component: Toaster,
  argTypes: {
    position: { control: 'select', options: ['top-left','top-center','top-right','bottom-left','bottom-center','bottom-right'] },
  },
  args: {
    position: 'bottom-right',
  },
} satisfies Meta<typeof Toaster>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: ({ position }) => (
    <div style={{ display: 'grid', gap: 8 }}>
      <div style={{ display: 'flex', gap: 8 }}>
        <Button onClick={() => toast.success('Saved successfully')}>Success</Button>
        <Button onClick={() => toast.info('Heads up!')}>Info</Button>
        <Button onClick={() => toast.warning('Be careful!')}>Warning</Button>
        <Button variant="destructive" onClick={() => toast.error('Something went wrong')}>Error</Button>
      </div>
      <div>
        <Button variant="outline" onClick={() => toast.promise(new Promise((res) => setTimeout(res, 1500)), { loading: 'Loading…', success: 'Done!', error: 'Failed' })}>Promise</Button>
      </div>
      <Toaster position={position as any} />
    </div>
  ),
};

