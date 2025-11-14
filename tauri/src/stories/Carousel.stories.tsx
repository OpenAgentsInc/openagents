import type { Meta, StoryObj } from '@storybook/react-vite';
import {
  Carousel,
  CarouselContent,
  CarouselItem,
  CarouselNext,
  CarouselPrevious,
} from "@openagentsinc/ui";

const meta = {
  title: 'UI/Carousel',
  component: Carousel,
  argTypes: {
    orientation: { control: 'select', options: ['horizontal', 'vertical'] },
    slides: { control: 'number' },
    width: { control: 'number' },
    height: { control: 'number' },
  },
  args: {
    orientation: 'horizontal',
    slides: 5,
    width: 520,
    height: 200,
  },
} satisfies Meta<typeof Carousel>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: ({ orientation, slides, width, height }) => {
    const count = Math.max(1, Number(slides));
    const style: React.CSSProperties = {
      display: 'grid',
      placeContent: 'center',
      border: '1px solid var(--border)',
      borderRadius: 6,
      background: 'var(--muted)',
      color: 'var(--muted-foreground)',
      height: orientation === 'horizontal' ? Number(height) : Math.max(160, Number(height)),
    };
    return (
      <div style={{ width: orientation === 'horizontal' ? Number(width) : 'auto', height: orientation === 'vertical' ? Number(height) : 'auto' }}>
        <Carousel orientation={orientation as any}>
          <CarouselContent>
            {Array.from({ length: count }).map((_, i) => (
              <CarouselItem key={i}>
                <div style={style}>Slide {i + 1}</div>
              </CarouselItem>
            ))}
          </CarouselContent>
          <CarouselPrevious />
          <CarouselNext />
        </Carousel>
      </div>
    );
  },
};

