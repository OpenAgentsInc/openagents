import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Terminal } from "lucide-react";


interface CreateWalletDisclaimerScreenProps {
  onNext: () => void;
}

const CreateWalletDisclaimerScreen: React.FC<CreateWalletDisclaimerScreenProps> = ({ onNext }) => {
  return (
    <div className="flex items-center justify-center min-h-screen bg-background p-4">
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