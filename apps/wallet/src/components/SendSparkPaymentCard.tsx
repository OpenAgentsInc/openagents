import React from 'react';
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card';
import { Input as UiInput } from '@/components/ui/input';
import { Button as UiButton } from '@/components/ui/button';
import { Label } from '@/components/ui/label';
import { Loader2, Sparkles } from 'lucide-react'; // Using Sparkles for Spark icon

interface SendSparkPaymentCardProps {
  recipientSparkAddress: string;
  setRecipientSparkAddress: (address: string) => void;
  sendSparkAmount: bigint;
  setSendSparkAmount: (amount: bigint) => void;
  handleSendSparkPayment: () => Promise<void>;
  isSendingSparkPayment: boolean;
  disabled: boolean;
}

const SendSparkPaymentCard: React.FC<SendSparkPaymentCardProps> = ({
  recipientSparkAddress,
  setRecipientSparkAddress,
  sendSparkAmount,
  setSendSparkAmount,
  handleSendSparkPayment,
  isSendingSparkPayment,
  disabled
}) => {
  const handleAmountChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const value = e.target.value;
    // Allow empty string for clearing, otherwise parse as BigInt
    setSendSparkAmount(value === '' ? BigInt(0) : BigInt(value.replace(/[^0-9]/g, '')));
  };

  // Allow both "sp1p" and "sprt1p" formats
  const trimmedAddress = recipientSparkAddress.trim();
  const isFormValid = (trimmedAddress.startsWith("sp1p") || trimmedAddress.startsWith("sprt1p")) && sendSparkAmount > BigInt(0);

  return (
    <Card className="mt-6">
      <CardHeader>
        <CardTitle>Send to Spark Address</CardTitle>
        <CardDescription>Send Bitcoin instantly to another Spark wallet.</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        <div className="space-y-2">
          <Label htmlFor="recipient-spark-address">Recipient's Spark Address</Label>
          <UiInput
            id="recipient-spark-address"
            type="text"
            placeholder="sp1p..."
            value={recipientSparkAddress}
            onChange={(e) => setRecipientSparkAddress(e.target.value)}
            disabled={isSendingSparkPayment || disabled}
          />
        </div>
        <div className="space-y-2">
          <Label htmlFor="send-spark-amount">Amount (sats)</Label>
          <UiInput
            id="send-spark-amount"
            type="number" // Use number for easier input, convert to BigInt in handler
            value={sendSparkAmount.toString()}
            onChange={handleAmountChange}
            min="1"
            disabled={isSendingSparkPayment || disabled}
          />
        </div>
        <div className="flex justify-center">
          <UiButton
            onClick={handleSendSparkPayment}
            disabled={isSendingSparkPayment || disabled || !isFormValid}
          >
            {isSendingSparkPayment ? (
              <>
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                Sending...
              </>
            ) : (
              <>
                <Sparkles className="mr-2 h-4 w-4" /> {/* Spark icon */}
                Send Spark Payment
              </>
            )}
          </UiButton>
        </div>
      </CardContent>
    </Card>
  );
};

export default SendSparkPaymentCard;