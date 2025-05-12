import { Button } from "@/components/ui/button";
import { Github } from "lucide-react";

interface LoginScreenProps {
  onCreateWallet: () => void;
  onEnterSeed: () => void;
}

const LoginScreen: React.FC<LoginScreenProps> = ({ onCreateWallet, onEnterSeed }) => {
  return (
    <div className="flex flex-col items-center justify-center min-h-screen bg-background text-foreground p-4">
      <div className="flex flex-col items-center text-center">
        <img src="/oalogo.png" alt="OpenAgents Logo" className="w-20 h-20 mb-8" />
        <div className="flex flex-col space-y-4 w-full max-w-xs">
          <Button onClick={onCreateWallet} className="w-full" size="lg">
            Create New Wallet
          </Button>
          <Button onClick={onEnterSeed} variant="outline" className="w-full" size="lg">
            Enter Seed Phrase
          </Button>
        </div>
      </div>
      <footer className="absolute bottom-6 text-center text-xs text-muted-foreground">
        An{" "}
        <a
          href="https://github.com/OpenAgentsInc/openagents/tree/main/apps/wallet"
          target="_blank"
          rel="noopener noreferrer"
          className="underline hover:text-primary inline-flex items-center"
        >
          open source
        </a>
        {" "}self-custodial bitcoin wallet powered by <a href="https://www.spark.money/" target="_blank" rel="noopener noreferrer" className="underline hover:text-primary">Spark</a>
      </footer>
    </div>
  );
};

export default LoginScreen;
