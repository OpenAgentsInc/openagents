import { useState, useEffect } from 'react';
import { useRelayConfigContext } from '@/contexts/RelayConfigContext';
import { getQueryClient } from '@/lib/queryClient';
import { normalizeRelayUrl, type RelayEntry } from '@/lib/relayConfig';
import { Button } from '@/components/ui/button';
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible';
import { ChevronDown, ChevronRight, Plus, Trash2 } from 'lucide-react';

const INDEXED_DB_NAME = 'openagents-events-v1';

export function RelaySettings() {
  const { relayMetadata, setRelayMetadata } = useRelayConfigContext();
  const [open, setOpen] = useState(false);
  const [editing, setEditing] = useState<RelayEntry[]>(relayMetadata.relays);
  const [dirty, setDirty] = useState(false);
  const [resetting, setResetting] = useState(false);

  useEffect(() => {
    if (open && !dirty) setEditing(relayMetadata.relays);
  }, [open, relayMetadata.relays, dirty]);

  function addRow() {
    setEditing((prev) => [
      ...prev,
      { url: 'wss://', read: true, write: true },
    ]);
    setDirty(true);
  }

  function removeRow(i: number) {
    setEditing((prev) => prev.filter((_, j) => j !== i));
    setDirty(true);
  }

  function changeRow(i: number, value: string) {
    setEditing((prev) => {
      const next = [...prev];
      next[i] = { ...next[i], url: value };
      return next;
    });
    setDirty(true);
  }

  function toggleMode(i: number, mode: 'read' | 'write') {
    setEditing((prev) => {
      const next = [...prev];
      const entry = next[i];
      if (!entry) return prev;
      const updated = { ...entry, [mode]: !entry[mode] };
      if (!updated.read && !updated.write) {
        updated.read = true;
      }
      next[i] = updated;
      return next;
    });
    setDirty(true);
  }

  function save() {
    const deduped = new Map<string, RelayEntry>();
    for (const entry of editing) {
      const normalized = normalizeRelayUrl(entry.url);
      if (!normalized) continue;
      const hasMode = entry.read || entry.write;
      deduped.set(normalized, {
        url: normalized,
        read: hasMode ? entry.read : true,
        write: hasMode ? entry.write : true,
      });
    }
    const relays = [...deduped.values()];
    setRelayMetadata({
      relays: relays.length > 0 ? relays : relayMetadata.relays,
      updatedAt: Math.floor(Date.now() / 1000),
    });
    setDirty(false);
    setOpen(false);
  }

  async function resetCache() {
    setResetting(true);
    try {
      if (typeof indexedDB !== 'undefined') {
        indexedDB.deleteDatabase(INDEXED_DB_NAME);
      }
      if (typeof localStorage !== 'undefined') {
        localStorage.removeItem('clawstr-query-cache-v1');
      }
      const client = getQueryClient();
      client.clear();
    } finally {
      setResetting(false);
    }
  }

  return (
    <Collapsible
      open={open}
      onOpenChange={setOpen}
      className="rounded-lg border border-border"
    >
      <CollapsibleTrigger className="flex w-full items-center gap-2 px-3 py-2 text-sm font-medium text-muted-foreground hover:text-foreground">
        {open ? (
          <ChevronDown className="size-4" />
        ) : (
          <ChevronRight className="size-4" />
        )}
        Relays
      </CollapsibleTrigger>
      <CollapsibleContent>
        <div className="border-t border-border p-3 space-y-2">
          {editing.map((relay, i) => (
            <div key={i} className="flex flex-wrap items-center gap-2">
              <input
                type="url"
                value={relay.url}
                onChange={(e) => changeRow(i, e.target.value)}
                placeholder="wss://..."
                className="border-input bg-background flex-1 min-w-[180px] rounded-md border px-2 py-1.5 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
              />
              <div className="flex items-center gap-1">
                <button
                  type="button"
                  onClick={() => toggleMode(i, 'read')}
                  className={`rounded-md border px-2 py-1 text-xs font-medium ${
                    relay.read
                      ? 'border-primary/40 bg-primary/10 text-primary'
                      : 'border-border text-muted-foreground hover:text-foreground'
                  }`}
                >
                  Read
                </button>
                <button
                  type="button"
                  onClick={() => toggleMode(i, 'write')}
                  className={`rounded-md border px-2 py-1 text-xs font-medium ${
                    relay.write
                      ? 'border-primary/40 bg-primary/10 text-primary'
                      : 'border-border text-muted-foreground hover:text-foreground'
                  }`}
                >
                  Write
                </button>
              </div>
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
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
            <span>
              Read relays are used for fetching; write relays for publishing.
            </span>
            <button
              type="button"
              onClick={() => void resetCache()}
              disabled={resetting}
              className="text-xs text-muted-foreground hover:text-foreground underline-offset-2 hover:underline disabled:opacity-50"
            >
              {resetting ? 'Resetting…' : 'Reset cache'}
            </button>
          </div>
        </div>
      </CollapsibleContent>
    </Collapsible>
  );
}
