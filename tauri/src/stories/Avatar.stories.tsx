import type { Meta, StoryObj } from '@storybook/react-vite';
import { Avatar, AvatarImage, AvatarFallback } from '@/components/ui/avatar';

type SizeKey = 'sm' | 'md' | 'lg' | 'xl';
const sizeClass: Record<SizeKey, string> = {
  sm: 'size-8',
  md: 'size-12',
  lg: 'size-16',
  xl: 'size-20',
};

const meta = {
  title: 'UI/Avatar',
  component: Avatar,
  argTypes: {
    size: {
      control: 'select',
      options: Object.keys(sizeClass),
    },
    src: { control: 'text' },
    initials: { control: 'text' },
  },
  args: {
    size: 'lg' as SizeKey,
    src: 'https://i.pravatar.cc/100?img=3',
    initials: 'AB',
  },
} satisfies Meta<typeof Avatar>;

export default meta;
type Story = StoryObj<typeof meta>;

export const WithImage: Story = {
  render: ({ size, src, initials }) => (
    <Avatar className={sizeClass[size as SizeKey]}>
      <AvatarImage src={src} alt="Avatar" />
      <AvatarFallback>{initials}</AvatarFallback>
    </Avatar>
  ),
};

export const WithFallback: Story = {
  args: { src: '' },
  render: ({ size, initials }) => (
    <Avatar className={sizeClass[size as SizeKey]}>
      <AvatarFallback>{initials}</AvatarFallback>
    </Avatar>
  ),
};
