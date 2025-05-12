import React from 'react';
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card';
import { Input as UiInput } from '@/components/ui/input'; // Renamed to avoid conflict
import { Button as UiButton } from '@/components/ui/button'; // Renamed to avoid conflict
import { Label } from '@/components/ui/label';
import { Loader2, Send } from 'lucide-react';

interface SendPaymentCardProps {
  sendInvoice: string;
  setSendInvoice: (invoice: string) => void;
  handlePayInvoice: () => Promise<void>;
  isSendingPayment: boolean;
  disabled: boolean; // To disable when SDK is not ready
}

const SendPaymentCard: React.FC<SendPaymentCardProps> = ({
  sendInvoice,
  setSendInvoice,
  handlePayInvoice,
  isSendingPayment,
  disabled
}) => {
  return (
    <Card className="mt-6">
      <CardHeader>
        <CardTitle>Send Payment (Lightning)</CardTitle>
        <CardDescription>Pay a Lightning invoice.</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        <div className="space-y-2">
          <Label htmlFor="send-invoice">Lightning Invoice (BOLT11)</Label>
          <UiInput
            id="send-invoice"
            type="text"
            placeholder="lnbc..."
            value={sendInvoice}
            onChange={(e) => setSendInvoice(e.target.value)}
            disabled={isSendingPayment || disabled}
          />
        </div>
        <div className="flex justify-center">
          <UiButton
            onClick={handlePayInvoice}
            disabled={isSendingPayment || disabled || !sendInvoice.trim()}
          >
            {isSendingPayment ? (
              <>
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                Sending...
              </>
            ) : (
              <>
                <Send className="mr-2 h-4 w-4" />
                Pay Invoice
              </>
            )}
          </UiButton>
        </div>
      </CardContent>
    </Card>
  );
};

export default SendPaymentCard;