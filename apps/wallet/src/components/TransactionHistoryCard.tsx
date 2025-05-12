import React from 'react';
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card';
import { ScrollArea } from '@/components/ui/scroll-area';
import TransactionItem from './TransactionItem';
import { SparkTransferData } from '../App';
import { ListCollapse } from 'lucide-react';

interface TransactionHistoryCardProps {
  transactions: SparkTransferData[];
}

const TransactionHistoryCard: React.FC<TransactionHistoryCardProps> = ({ transactions }) => {
  return (
    <Card className="mt-6">
      <CardHeader>
        <CardTitle>Transaction History</CardTitle>
        <CardDescription>Your recent Bitcoin transactions.</CardDescription>
      </CardHeader>
      <CardContent>
        {transactions.length > 0 ? (
          <ScrollArea className="h-[300px] w-full rounded-md border">
            <div className="p-1">
              {transactions.filter(tx => tx && tx.id).map((tx) => (
                <TransactionItem key={tx.id} transaction={tx} />
              ))}
            </div>
          </ScrollArea>
        ) : (
          <div className="flex flex-col items-center justify-center h-[150px] text-center">
            <ListCollapse className="h-12 w-12 text-muted-foreground mb-2" />
            <p className="text-sm text-muted-foreground">No transactions yet.</p>
            <p className="text-xs text-muted-foreground">Your transactions will appear here once you send or receive Bitcoin.</p>
          </div>
        )}
      </CardContent>
    </Card>
  );
};

export default TransactionHistoryCard;