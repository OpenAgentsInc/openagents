import React from 'react';
import { SparkTransferData } from '../App';
import { ArrowUpRight, ArrowDownLeft } from 'lucide-react';
import { Badge } from '@/components/ui/badge';
import { cn } from '@/lib/utils';

interface TransactionItemProps {
  transaction: SparkTransferData;
}

const TransactionItem: React.FC<TransactionItemProps> = ({ transaction }) => {
  // Initialize determination of whether this is a sent transaction
  let isSent = false;
  
  // First check explicit direction fields (highest priority)
  if (transaction?.transfer_direction === "OUTGOING" || transaction?.transferDirection === "OUTGOING") {
    isSent = true;
  } else if (transaction?.transfer_direction === "INCOMING" || transaction?.transferDirection === "INCOMING") {
    isSent = false;
  }
  
  // Amount-based direction check (second priority)
  // Negative amounts always indicate sent transactions
  if (transaction?.totalValue) {
    const value = typeof transaction.totalValue === 'bigint' 
      ? transaction.totalValue 
      : BigInt(transaction.totalValue);
    
    if (value < BigInt(0)) {
      isSent = true;
    } else if (value > BigInt(0) && !transaction?.transfer_direction && !transaction?.transferDirection) {
      isSent = false;
    }
  }
  
  // Type-based specialized determination (third priority)
  if (transaction?.type) {
    const txType = transaction.type.toUpperCase();
    
    // For PREIMAGE_SWAP (Lightning), typically it's a receive 
    if (txType === "PREIMAGE_SWAP") {
      isSent = false; // Override - PREIMAGE_SWAP is always a received payment
    }
    
    // For explicit Lightning payments, check specific handling
    if (txType.includes("LIGHTNING") && txType.includes("PAYMENT")) {
      // Further analyze Lightning payments based on context
      // If we have a description containing "paid invoice", it's sent
      if (transaction.description && transaction.description.toLowerCase().includes("paid invoice")) {
        isSent = true;
      }
    }
  }
  
  // Log transaction data for debugging
  console.log("Transaction data:", {
    id: transaction?.id,
    type: transaction?.type,
    direction: transaction?.transfer_direction || transaction?.transferDirection,
    totalValue: transaction?.totalValue,
    total_sent: transaction?.total_sent,
    amount: transaction?.amount,
    status: transaction?.status,
    determinedDirection: isSent ? "OUTGOING" : "INCOMING"
  });
  
  // Handle different possible amount fields 
  let amount = BigInt(0);
  
  // Try to find the amount from various possible fields
  if (transaction?.totalValue) {
    // The logs showed totalValue is the field we need
    const rawAmount = typeof transaction.totalValue === 'bigint' 
      ? transaction.totalValue 
      : BigInt(transaction.totalValue);
    
    // Always store the absolute value for display
    amount = rawAmount < BigInt(0) ? -rawAmount : rawAmount;
  } else if (transaction?.total_sent) {
    amount = typeof transaction.total_sent === 'bigint'
      ? transaction.total_sent
      : BigInt(transaction.total_sent);
  } else if (transaction?.amount) {
    amount = typeof transaction.amount === 'bigint'
      ? transaction.amount
      : BigInt(transaction.amount);
  } else if (transaction?.amountSat) {
    amount = typeof transaction.amountSat === 'bigint'
      ? transaction.amountSat
      : BigInt(transaction.amountSat);
  } else if (transaction?.invoice?.amount?.amountSat) {
    amount = typeof transaction.invoice.amount.amountSat === 'bigint'
      ? transaction.invoice.amount.amountSat
      : BigInt(transaction.invoice.amount.amountSat);
  }
  
  // Format the amount with appropriate decimal places (if needed)
  // Currently just showing raw sats with prefixed sign
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
  } catch {
    // Keep the default "Just now" if we couldn't parse the date
  }

  const getStatusBadgeVariant = (status: string | undefined): "default" | "secondary" | "destructive" | "outline" => {
    if (!status) return "outline";
    
    const upperStatus = status.toUpperCase();
    
    // Check for completed/success states
    if (upperStatus === 'COMPLETED' || 
        upperStatus === 'SUCCESS' || 
        upperStatus === 'CONFIRMED' ||
        upperStatus === 'TRANSFER_STATUS_COMPLETE') {
      return "default"; // Default is usually primary color
    }
    
    // Check for pending/in-progress states
    if (upperStatus === 'PENDING' || 
        upperStatus.includes('INITIATED') ||
        upperStatus.includes('PROCESSING') ||
        upperStatus === 'TRANSFER_STATUS_SENDER_KEY_TWEAKED' ||
        upperStatus === 'LIGHTNING_PAYMENT_INITIATED' ||
        upperStatus.includes('IN_PROGRESS')) {
      return "secondary";
    }
    
    // Check for failed/error states
    if (upperStatus === 'FAILED' || 
        upperStatus.includes('ERROR') || 
        upperStatus.includes('REJECTED') ||
        upperStatus === 'TRANSFER_STATUS_RETURNED' ||
        upperStatus.includes('CANCELLED')) {
      return "destructive";
    }
    
    // Default to outline for unknown statuses
    return "outline";
  }
  
  // Format status text to be more human-readable
  const formatStatus = (status: string | undefined): string => {
    if (!status) return 'Unknown';
    
    // Remove common prefixes
    let formattedStatus = status
      .replace(/transfer_status_/i, '')
      .replace(/transfer status /i, '')
      .replace(/lightning_payment_/i, '')
      .replace(/lightning payment /i, '')
      .replace(/_/g, ' ')
      .toLowerCase();
    
    // Capitalize first letter of each word
    formattedStatus = formattedStatus
      .split(' ')
      .map(word => word.charAt(0).toUpperCase() + word.slice(1))
      .join(' ');
    
    return formattedStatus;
  }

  const formatType = (type: string | undefined): string => {
    if (!type) return "Unknown Transaction";
    
    // Convert to uppercase for consistent comparison
    const uppercaseType = type.toUpperCase();
    
    // Handle specific known types with fixed labels
    if (uppercaseType === "PREIMAGE_SWAP") {
      return "Lightning Payment Received"; // Always a receive
    }
    
    if (uppercaseType === "SPARK_TRANSFER") {
      return isSent ? "Spark Transfer Sent" : "Spark Transfer Received";
    }
    
    // For Lightning payments, be explicit about direction
    if (uppercaseType.includes("LIGHTNING") && uppercaseType.includes("PAYMENT")) {
      return isSent ? "Lightning Payment Sent" : "Lightning Payment Received";
    }
    
    // For On-chain transactions, explicitly handle direction
    if (uppercaseType.includes("ONCHAIN") || uppercaseType.includes("ON_CHAIN") || uppercaseType.includes("ON-CHAIN")) {
      return isSent ? "On-chain Payment Sent" : "On-chain Payment Received";
    }
    
    // For any other type, use consistent naming pattern
    const cleanType = type.replace(/_/g, ' ').toLowerCase();
    
    // Extract the network/protocol type (Lightning, On-chain, etc.)
    let network = "Spark"; // Default to Spark
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
    } else if (cleanType.includes("swap")) {
      action = "Swap";
    } else if (cleanType.includes("invoice")) {
      action = "Invoice";
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
          {formatStatus(transaction?.status)}
        </Badge>
      </div>
    </div>
  );
};

export default TransactionItem;