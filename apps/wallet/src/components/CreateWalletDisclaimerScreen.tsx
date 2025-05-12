import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Terminal, ArrowLeft } from "lucide-react";


interface CreateWalletDisclaimerScreenProps {
  onNext: () => void;
  onBack: () => void;
}

const CreateWalletDisclaimerScreen: React.FC<CreateWalletDisclaimerScreenProps> = ({ onNext, onBack }) => {
  return (
    <div className="flex flex-col items-center justify-center min-h-screen bg-background p-4">
      <div className="w-full max-w-md mb-4">
        <button 
          onClick={onBack}
          className="text-sm flex items-center text-muted-foreground hover:text-primary transition-colors"
        >
          <ArrowLeft className="mr-1 h-4 w-4" /> Back to Home
        </button>
      </div>
      <Card className="w-full max-w-md">
        <CardHeader>
          <CardTitle>Important Notice</CardTitle>
          <CardDescription>Please read carefully before proceeding.</CardDescription>
        </CardHeader>
        <CardContent>
          <Alert variant="destructive">
            <Terminal className="h-4 w-4" />
            <AlertTitle>Self-Custody Wallet</AlertTitle>
            <AlertDescription>
              OpenAgents wallet is self-custodial. OpenAgents cannot access your funds or help recover them if lost. You are solely responsible for securing your seed phrase.
            </AlertDescription>
          </Alert>
        </CardContent>
        <CardFooter>
          <Button onClick={onNext} className="w-full">
            I Understand, Continue
          </Button>
        </CardFooter>
      </Card>
    </div>
  );
};

export default CreateWalletDisclaimerScreen;