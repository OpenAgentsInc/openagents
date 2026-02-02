import { useState, useEffect } from "react";
import { useRelayConfigContext } from "@/contexts/RelayConfigContext";
import { getQueryClient } from "@/lib/queryClient";
import { Button } from "@/components/ui/button";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { ChevronDown, ChevronRight, Plus, Trash2 } from "lucide-react";

export function RelaySettings() {
  const { relayUrls, setRelayUrls } = useRelayConfigContext();
  const [open, setOpen] = useState(false);
  const [editing, setEditing] = useState<string[]>(relayUrls);
  const [dirty, setDirty] = useState(false);
  const [resetting, setResetting] = useState(false);

  useEffect(() => {
    if (open && !dirty) setEditing(relayUrls);
  }, [open, relayUrls, dirty]);

  function addRow() {
    setEditing((prev) => [...prev, "wss://"]);
    setDirty(true);
  }

  function removeRow(i: number) {
    setEditing((prev) => prev.filter((_, j) => j !== i));
    setDirty(true);
  }

  function changeRow(i: number, value: string) {
    setEditing((prev) => {
      const next = [...prev];
      next[i] = value;
      return next;
    });
    setDirty(true);
  }

  function save() {
    const valid = editing.filter((u) => u.startsWith("wss://") && u.length > 6);
    setRelayUrls(valid.length > 0 ? valid : relayUrls);
    setDirty(false);
    setOpen(false);
  }

  async function resetCache() {
    setResetting(true);
    try {
      if (typeof indexedDB !== "undefined") {
        indexedDB.deleteDatabase("clawstr-events-v1");
      }
      if (typeof localStorage !== "undefined") {
        localStorage.removeItem("clawstr-query-cache-v1");
      }
      const client = getQueryClient();
      client.clear();
    } finally {
      setResetting(false);
    }
  }

  return (
    <Collapsible open={open} onOpenChange={setOpen} className="rounded-lg border border-border">
      <CollapsibleTrigger className="flex w-full items-center gap-2 px-3 py-2 text-sm font-medium text-muted-foreground hover:text-foreground">
        {open ? <ChevronDown className="size-4" /> : <ChevronRight className="size-4" />}
        Relays
      </CollapsibleTrigger>
      <CollapsibleContent>
        <div className="border-t border-border p-3 space-y-2">
          {editing.map((url, i) => (
            <div key={i} className="flex gap-2">
              <input
                type="url"
                value={url}
                onChange={(e) => changeRow(i, e.target.value)}
                placeholder="wss://..."
                className="border-input bg-background flex-1 rounded-md border px-2 py-1.5 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
              />
              <Button
                type="button"
                variant="ghost"
                size="icon-xs"
                onClick={() => removeRow(i)}
                aria-label="Remove relay"
              >
                <Trash2 className="size-3.5" />
              </Button>
            </div>
          ))}
          <div className="flex gap-2">
            <Button type="button" variant="outline" size="sm" onClick={addRow}>
              <Plus className="size-3.5" /> Add
            </Button>
            {dirty && (
              <Button type="button" size="sm" onClick={save}>
                Save relays
              </Button>
            )}
          </div>
          <div className="flex items-center justify-between text-xs text-muted-foreground">
            <span>Relays are stored in this browser. Refresh to use new list.</span>
            <button
              type="button"
              onClick={() => void resetCache()}
              disabled={resetting}
              className="text-xs text-muted-foreground hover:text-foreground underline-offset-2 hover:underline disabled:opacity-50"
            >
              {resetting ? "Resetting…" : "Reset cache"}
            </button>
          </div>
        </div>
      </CollapsibleContent>
    </Collapsible>
  );
}
