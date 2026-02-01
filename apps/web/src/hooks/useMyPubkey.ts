import { useEffect, useState } from "react";
import { NBrowserSigner } from "@nostrify/nostrify";
import { hasNostrExtension } from "@/lib/publishKind1111";

type PubkeyStatus = "idle" | "loading" | "ready" | "error";

export function useMyPubkey() {
  const [pubkey, setPubkey] = useState<string | null>(null);
  const [status, setStatus] = useState<PubkeyStatus>("idle");
  const hasExtension = hasNostrExtension();

  useEffect(() => {
    let active = true;
    if (!hasExtension) {
      setPubkey(null);
      setStatus("idle");
      return () => {};
    }
    setStatus("loading");
    const signer = new NBrowserSigner();
    signer
      .getPublicKey()
      .then((key) => {
        if (!active) return;
        setPubkey(key);
        setStatus("ready");
      })
      .catch(() => {
        if (!active) return;
        setStatus("error");
      });
    return () => {
      active = false;
    };
  }, [hasExtension]);

  return { pubkey, status, hasExtension };
}
