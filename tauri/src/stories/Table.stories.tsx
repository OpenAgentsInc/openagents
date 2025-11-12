import type { Meta, StoryObj } from '@storybook/react-vite';
import {
  Table,
  TableHeader,
  TableBody,
  TableRow,
  TableHead,
  TableCell,
  TableCaption,
  TableFooter,
} from '@/components/ui/table';

type Row = { name: string; email: string; role: string };
const rows: Row[] = [
  { name: 'Ada Lovelace', email: 'ada@example.com', role: 'Admin' },
  { name: 'Alan Turing', email: 'alan@example.com', role: 'Member' },
  { name: 'Grace Hopper', email: 'grace@example.com', role: 'Owner' },
  { name: 'Edsger Dijkstra', email: 'edsger@example.com', role: 'Member' },
];

const meta = {
  title: 'UI/Table',
  component: Table,
  argTypes: {
    width: { control: 'number' },
  },
  args: {
    width: 720,
  },
} satisfies Meta<typeof Table>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: ({ width }) => (
    <div style={{ width: Number(width) }}>
      <Table>
        <TableCaption>Team members</TableCaption>
        <TableHeader>
          <TableRow>
            <TableHead>Name</TableHead>
            <TableHead>Email</TableHead>
            <TableHead>Role</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {rows.map((r) => (
            <TableRow key={r.email}>
              <TableCell>{r.name}</TableCell>
              <TableCell>{r.email}</TableCell>
              <TableCell>{r.role}</TableCell>
            </TableRow>
          ))}
        </TableBody>
        <TableFooter>
          <TableRow>
            <TableCell colSpan={3}>Total: {rows.length} members</TableCell>
          </TableRow>
        </TableFooter>
      </Table>
    </div>
  ),
};

