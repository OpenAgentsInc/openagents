import React from 'react';
import { SparkTransferData } from '../App';
import { ArrowUpRight, ArrowDownLeft } from 'lucide-react';
import { Badge } from '@/components/ui/badge';
import { cn } from '@/lib/utils';

interface TransactionItemProps {
  transaction: SparkTransferData;
}

const TransactionItem: React.FC<TransactionItemProps> = ({ transaction }) => {
  // Debug log to understand the transaction structure
  console.log("Transaction data:", {
    id: transaction?.id,
    type: transaction?.type,
    direction: transaction?.transfer_direction || transaction?.transferDirection,
    totalValue: transaction?.totalValue,
    total_sent: transaction?.total_sent,
    amount: transaction?.amount,
    status: transaction?.status
  });
  
  // Check both possible field names for the transfer direction
  // Also consider the transaction type for special cases like transfers
  let isSent = transaction?.transfer_direction === "OUTGOING" || transaction?.transferDirection === "OUTGOING";
  
  // If it's a transfer type and the direction isn't explicitly set, check for other indicators
  if (transaction?.type && transaction.type.toLowerCase().includes("transfer")) {
    // For transfers, ensure we're correctly marking sends vs receives
    if (transaction.type === "SPARK_TRANSFER" && transaction.totalValue && transaction.totalValue < 0) {
      isSent = true;
    }
  }
  
  // Handle different possible amount fields 
  let amount = BigInt(0);
  
  // Try to find the amount from various possible fields
  if (transaction?.totalValue) {
    // The logs showed totalValue is the field we need
    let rawAmount = typeof transaction.totalValue === 'bigint' 
      ? transaction.totalValue 
      : BigInt(transaction.totalValue);
    
    // For totalValue, we need to handle the sign correctly
    // If it's negative and outgoing, make it positive for display
    if (rawAmount < BigInt(0)) {
      isSent = true; // Force it to be sent if amount is negative
      amount = -rawAmount; // Make it positive for display
    } else {
      amount = rawAmount;
    }
  } else if (transaction?.total_sent) {
    amount = transaction.total_sent;
  } else if (transaction?.amount) {
    amount = BigInt(transaction.amount);
  } else if (transaction?.amountSat) {
    amount = BigInt(transaction.amountSat);
  } else if (transaction?.invoice?.amount?.amountSat) {
    amount = BigInt(transaction.invoice.amount.amountSat);
  }
  
  // Make sure amount is displayed as positive with the appropriate sign prefix
  const amountDisplay = `₿ ${isSent ? '-' : '+'}${amount.toString()}`;

  let dateDisplay = "Just now"; // Default to "Just now" instead of "Date unknown"
  try {
    // From the logs, we found that createdTime is the field we need
    const dateField = transaction?.createdTime || 
                     transaction?.updatedTime || 
                     transaction?.created_at_time || 
                     transaction?.updated_at_time || 
                     transaction?.created_at || 
                     transaction?.timestamp;
    
    if (dateField) {
      // Handle Unix timestamp (number) or ISO string
      const dateObj = typeof dateField === 'number' 
        ? new Date(dateField * 1000) // If it's a Unix timestamp (seconds)
        : new Date(dateField);       // If it's an ISO string
      
      if (!isNaN(dateObj.getTime())) {
        dateDisplay = dateObj.toLocaleString(undefined, {
          year: 'numeric', month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit'
        });
      }
    }
  } catch (e) {
    // Keep the default "Just now" if we couldn't parse the date
  }

  const getStatusBadgeVariant = (status: string | undefined): "default" | "secondary" | "destructive" | "outline" => {
    if (!status) return "outline";
    
    switch (status.toUpperCase()) {
      case 'COMPLETED':
        return "default"; // Default is usually primary color
      case 'PENDING':
      case 'TRANSFER_STATUS_SENDER_KEY_TWEAKED': // Example pending statuses
      case 'LIGHTNING_PAYMENT_INITIATED':
        return "secondary";
      case 'FAILED':
      case 'TRANSFER_STATUS_RETURNED':
        return "destructive";
      default:
        return "outline";
    }
  }

  const formatType = (type: string | undefined): string => {
    if (!type) return "Unknown Transaction";
    
    // First handle specific known types
    if (type.toUpperCase() === "PREIMAGE_SWAP") {
      return "Lightning Payment Received";
    }
    
    if (type === "SPARK_TRANSFER") {
      return isSent ? "Spark Transfer Sent" : "Spark Transfer Received";
    }
    
    // For any other type, create consistent naming pattern
    const cleanType = type.replace(/_/g, ' ').toLowerCase();
    
    // Extract the network/protocol type (Lightning, On-chain, etc.)
    let network = "Spark"; // Changed default to Spark since most non-lightning transfers are Spark transfers
    if (cleanType.includes("lightning")) {
      network = "Lightning";
    } else if (cleanType.includes("on-chain") || cleanType.includes("onchain")) {
      network = "On-chain";
    } else if (cleanType.includes("bitcoin") && !cleanType.includes("spark")) {
      network = "Bitcoin";
    }
    
    // Determine if it's a payment, transfer, or other action
    let action = "Transaction";
    if (cleanType.includes("payment")) {
      action = "Payment";
    } else if (cleanType.includes("transfer")) {
      action = "Transfer";
    } else if (cleanType.includes("withdraw")) {
      action = "Withdrawal";
    } else if (cleanType.includes("deposit")) {
      action = "Deposit";
    }
    
    // Create a consistent format: "[Network] [Action] [Direction]"
    return `${network} ${action} ${isSent ? "Sent" : "Received"}`;
  };

  const descriptionOrType = transaction?.description || formatType(transaction?.type);

  return (
    <div className="flex items-center justify-between p-3 border-b border-border last:border-b-0">
      <div className="flex items-center gap-3">
        {isSent ? (
          <ArrowUpRight className="h-5 w-5 text-destructive" />
        ) : (
          <ArrowDownLeft className="h-5 w-5 text-green-500" />
        )}
        <div>
          <p className="text-sm font-medium truncate max-w-[150px] sm:max-w-xs" title={descriptionOrType}>
            {descriptionOrType}
          </p>
          <p className="text-xs text-muted-foreground">{dateDisplay}</p>
        </div>
      </div>
      <div className="text-right">
        <p className={cn(
          "text-sm font-semibold",
          isSent ? "text-destructive" : "text-green-600 dark:text-green-500"
        )}>
          {amountDisplay}
        </p>
        <Badge variant={getStatusBadgeVariant(transaction?.status)} className="mt-1 text-xs">
          {transaction?.status 
            ? transaction.status.replace(/_/g, ' ').toLowerCase().replace('transfer status ', '').replace('lightning payment ','')
            : 'Unknown'
          }
        </Badge>
      </div>
    </div>
  );
};

export default TransactionItem;