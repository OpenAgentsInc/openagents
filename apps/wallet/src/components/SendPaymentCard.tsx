import React, { useState, useEffect } from 'react';
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '@/components/ui/card';
import { Input as UiInput } from '@/components/ui/input'; // Renamed to avoid conflict
import { Button as UiButton } from '@/components/ui/button'; // Renamed to avoid conflict
import { Label } from '@/components/ui/label';
import { Loader2, Send, AlertCircle } from 'lucide-react';

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
  const [validInvoice, setValidInvoice] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  // Basic validation of BOLT11 invoice format
  useEffect(() => {
    if (!sendInvoice.trim()) {
      setValidInvoice(false);
      setErrorMessage(null);
      return;
    }

    const trimmedInvoice = sendInvoice.trim();
    
    // Basic BOLT11 validation - Most Lightning invoices start with ln
    if (!(trimmedInvoice.toLowerCase().startsWith("ln")) || trimmedInvoice.length < 20) {
      setValidInvoice(false);
      setErrorMessage("Invalid invoice format. Lightning invoices must start with 'ln'");
      return;
    }

    // Check for common problematic characters that might have been added in copying
    if (trimmedInvoice.includes(" ") || trimmedInvoice.includes("\n") || trimmedInvoice.includes("\t")) {
      // Auto-clean the invoice by removing whitespace
      const cleanedInvoice = trimmedInvoice.replace(/\s+/g, '');
      setSendInvoice(cleanedInvoice);
      // Since we've cleaned it, we'll skip the validation error
      // The validation will run again on the next render with the cleaned invoice
      return;
    }

    setValidInvoice(true);
    setErrorMessage(null);
  }, [sendInvoice]);

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
            className={errorMessage ? "border-red-500 focus-visible:ring-red-500" : ""}
          />
          {errorMessage && (
            <div className="flex items-center mt-1 text-red-500 text-xs">
              <AlertCircle className="h-3 w-3 mr-1" />
              {errorMessage}
            </div>
          )}
          <p className="text-xs text-muted-foreground">
            Enter a valid Lightning invoice starting with 'ln'. Make sure to paste the entire invoice without any spaces or extra characters. The wallet will pay the exact amount specified in the invoice.
          </p>
          <p className="text-xs text-amber-600 dark:text-amber-400 mt-1">
            <strong>Note:</strong> Payment may fail if your device isn't connected to the internet or if the Lightning Network is experiencing issues. This is a beta feature.
          </p>
        </div>
        <div className="flex justify-center">
          <UiButton
            onClick={handlePayInvoice}
            disabled={isSendingPayment || disabled || !validInvoice}
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