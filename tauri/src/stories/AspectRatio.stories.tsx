import type { Meta, StoryObj } from '@storybook/react-vite';
import { AspectRatio } from '@/components/ui/aspect-ratio';

const meta = {
  title: 'UI/AspectRatio',
  component: AspectRatio,
  argTypes: {
    ratio: { control: 'number' },
    width: { control: 'number' },
    useImage: { control: 'boolean' },
  },
  args: {
    ratio: 16 / 9,
    width: 480,
    useImage: true,
  },
} satisfies Meta<typeof AspectRatio>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: ({ ratio, width, useImage }) => (
    <div style={{ width: Number(width) }}>
      <AspectRatio ratio={Number(ratio)}>
        {useImage ? (
          <img
            src={`https://picsum.photos/seed/oa-${Math.round(Number(ratio) * 100)}/1200/800`}
            alt="Random"
            style={{ width: '100%', height: '100%', objectFit: 'cover', borderRadius: 8 }}
          />
        ) : (
          <div style={{
            width: '100%', height: '100%', borderRadius: 8,
            display: 'grid', placeContent: 'center',
            background: 'linear-gradient(135deg, var(--muted) 0%, var(--card) 100%)',
            color: 'var(--muted-foreground)'
          }}>
            {Number(ratio).toFixed(2)} ratio
          </div>
        )}
      </AspectRatio>
    </div>
  ),
};

