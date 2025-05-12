import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Copy, ShieldAlert, ArrowLeft } from "lucide-react";
import { toast } from "sonner";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip"


interface ShowMnemonicScreenProps {
  mnemonic: string;
  onNext: () => void;
  onBack: () => void;
}

const ShowMnemonicScreen: React.FC<ShowMnemonicScreenProps> = ({ mnemonic, onNext, onBack }) => {
  const words = mnemonic.split(' ');

  const handleCopyMnemonic = () => {
    navigator.clipboard.writeText(mnemonic);
    toast.success("Seed Phrase Copied!", {
      description: "Your 12-word seed phrase has been copied to the clipboard.",
    });
  };

  return (
    <div className="flex flex-col items-center justify-center min-h-screen bg-background p-4">
      <div className="w-full max-w-lg mb-4">
        <button 
          onClick={onBack}
          className="text-sm flex items-center text-muted-foreground hover:text-primary transition-colors"
        >
          <ArrowLeft className="mr-1 h-4 w-4" /> Back to Home
        </button>
      </div>
      <Card className="w-full max-w-lg">
        <CardHeader>
          <CardTitle>Your Secret Recovery Phrase</CardTitle>
          <CardDescription>
            Write down these 12 words in order and keep them somewhere safe.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-6">
          <Alert variant="destructive">
            <ShieldAlert className="h-4 w-4" />
            <AlertTitle>Critical Warning!</AlertTitle>
            <AlertDescription>
              This is your password to your money. If you lose it, you will lose your money! Never share this phrase with anyone.
            </AlertDescription>
          </Alert>

          <div className="grid grid-cols-2 md:grid-cols-3 gap-3 p-4 border rounded-md bg-muted/50">
            {words.map((word, index) => (
              <div key={index} className="flex items-center p-2 bg-background border rounded-md">
                <span className="text-xs text-muted-foreground mr-2 select-none">{index + 1}.</span>
                <span className="font-medium">{word}</span>
              </div>
            ))}
          </div>

          <TooltipProvider>
            <Tooltip delayDuration={100}>
              <TooltipTrigger asChild>
                <Button onClick={handleCopyMnemonic} variant="outline" className="w-full">
                  <Copy className="mr-2 h-4 w-4" /> Copy Seed Phrase
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                <p>Copy all 12 words to clipboard</p>
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>

        </CardContent>
        <CardFooter>
          <Button onClick={onNext} className="w-full">
            I Have Saved My Seed Phrase, Next
          </Button>
        </CardFooter>
      </Card>
    </div>
  );
};

export default ShowMnemonicScreen;