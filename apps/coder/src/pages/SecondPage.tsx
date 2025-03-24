import React from "react";
import Footer from "@/components/template/Footer";
import { useTranslation } from "react-i18next";
import { Button } from "@openagents/ui";

export default function SecondPage() {
  const { t } = useTranslation();

  return (
    <div className="flex h-full flex-col">
      <div className="flex flex-1 flex-col items-center justify-center gap-2">
        <Button
          label="Test Button from UI"
          onPress={() => console.log("Button pressed")}
        />
      </div>
      <Footer />
    </div>
  );
}
