import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { toast } from "sonner";

interface EnterSeedScreenProps {
  onSeedEntered: (seed: string) => void;
}

const EnterSeedScreen: React.FC<EnterSeedScreenProps> = ({ onSeedEntered }) => {
  const [seedPhrase, setSeedPhrase] = useState('');

  const handleSubmit = () => {
    const trimmedSeed = seedPhrase.trim().toLowerCase();
    const words = trimmedSeed.split(/\s+/); // Split by any whitespace

    if (words.length !== 12 && words.length !== 24) { // Common lengths
      toast.error("Invalid Seed Phrase", {
        description: "Seed phrases usually have 12 or 24 words. Please check your input.",
      });
      return;
    }
    onSeedEntered(trimmedSeed);
  };

  return (
    <div className="flex items-center justify-center min-h-screen bg-background p-4">
      <Card className="w-full max-w-md">
        <CardHeader>
          <CardTitle>Enter Your Seed Phrase</CardTitle>
          <CardDescription>
            Enter your 12 or 24 word recovery phrase to restore your wallet. Separate words with spaces.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <div className="grid w-full gap-1.5">
            <Label htmlFor="seed-phrase">Seed Phrase</Label>
            <Textarea
              id="seed-phrase"
              placeholder="Enter your seed phrase here..."
              value={seedPhrase}
              onChange={(e) => setSeedPhrase(e.target.value)}
              rows={4}
              className="resize-none"
            />
          </div>
        </CardContent>
        <CardFooter>
          <Button onClick={handleSubmit} className="w-full">
            Restore Wallet
          </Button>
        </CardFooter>
      </Card>
    </div>
  );
};

export default EnterSeedScreen;