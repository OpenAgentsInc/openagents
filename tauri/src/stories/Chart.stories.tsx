import type { Meta, StoryObj } from '@storybook/react-vite';
import {
  ChartContainer,
  ChartLegend,
  ChartLegendContent,
  ChartTooltip,
  ChartTooltipContent,
  type ChartConfig,
} from "@openagentsinc/ui";
import {
  Area,
  AreaChart,
  CartesianGrid,
  XAxis,
  YAxis,
} from 'recharts';

type Datum = { month: string; visitors: number; signups: number };
const data: Datum[] = [
  { month: 'Jan', visitors: 1200, signups: 200 },
  { month: 'Feb', visitors: 1800, signups: 320 },
  { month: 'Mar', visitors: 2400, signups: 440 },
  { month: 'Apr', visitors: 2200, signups: 380 },
  { month: 'May', visitors: 2600, signups: 500 },
  { month: 'Jun', visitors: 3000, signups: 630 },
];

const config: ChartConfig = {
  visitors: { label: 'Visitors', color: 'hsl(var(--chart-1))' },
  signups: { label: 'Signups', color: 'hsl(var(--chart-2))' },
};

const meta = {
  title: 'UI/Chart',
  component: ChartContainer,
  argTypes: {
    showGrid: { control: 'boolean' },
    showLegend: { control: 'boolean' },
    showYAxis: { control: 'boolean' },
  },
  args: {
    showGrid: true,
    showLegend: true,
    showYAxis: true,
  },
} satisfies Meta<typeof ChartContainer>;

export default meta;
type Story = StoryObj<typeof meta>;

export const AreaBasic: Story = {
  render: ({ showGrid, showLegend, showYAxis }) => (
    <div style={{ width: 560 }}>
      <ChartContainer config={config}>
        <AreaChart data={data} margin={{ left: 12, right: 12 }}>
          {showGrid && <CartesianGrid vertical={false} strokeDasharray="3 3" />}
          <XAxis dataKey="month" tickLine={false} axisLine={false} />
          {showYAxis && <YAxis tickLine={false} axisLine={false} width={32} />}
          <ChartTooltip content={<ChartTooltipContent />} />
          {showLegend && (
            <ChartLegend content={<ChartLegendContent />} verticalAlign="bottom" />
          )}
          <Area type="monotone" dataKey="visitors" stroke="var(--color-visitors)" fill="var(--color-visitors)" fillOpacity={0.25} />
          <Area type="monotone" dataKey="signups" stroke="var(--color-signups)" fill="var(--color-signups)" fillOpacity={0.25} />
        </AreaChart>
      </ChartContainer>
    </div>
  ),
};

