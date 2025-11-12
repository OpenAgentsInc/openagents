import type { Meta, StoryObj } from '@storybook/react-vite';
import {
  Pagination,
  PaginationContent,
  PaginationItem,
  PaginationLink,
  PaginationPrevious,
  PaginationNext,
  PaginationEllipsis,
} from '@/components/ui/pagination';

const meta = {
  title: 'UI/Pagination',
  component: Pagination,
  argTypes: {
    page: { control: 'number' },
    total: { control: 'number' },
    width: { control: 'number' },
  },
  args: {
    page: 3,
    total: 10,
    width: 520,
  },
} satisfies Meta<typeof Pagination>;

export default meta;
type Story = StoryObj<typeof meta>;

function range(from: number, to: number) {
  return Array.from({ length: to - from + 1 }, (_, i) => from + i);
}

export const Basic: Story = {
  render: ({ page, total, width }) => {
    const p = Math.max(1, Math.min(Number(page), Number(total)));
    const t = Math.max(1, Number(total));
    const pages = t <= 7
      ? range(1, t)
      : p <= 4
      ? [1, 2, 3, 4, 5, NaN, t]
      : p >= t - 3
      ? [1, NaN, t - 4, t - 3, t - 2, t - 1, t]
      : [1, NaN, p - 1, p, p + 1, NaN, t];

    return (
      <div style={{ width: Number(width) }}>
        <Pagination>
          <PaginationContent>
            <PaginationItem>
              <PaginationPrevious href="#" />
            </PaginationItem>
            {pages.map((n, i) => (
              Number.isNaN(n) ? (
                <PaginationItem key={`e-${i}`}>
                  <PaginationEllipsis />
                </PaginationItem>
              ) : (
                <PaginationItem key={n}>
                  <PaginationLink href="#" isActive={n === p}>
                    {n}
                  </PaginationLink>
                </PaginationItem>
              )
            ))}
            <PaginationItem>
              <PaginationNext href="#" />
            </PaginationItem>
          </PaginationContent>
        </Pagination>
      </div>
    );
  },
};

