"use client";

import { useQuery } from "convex/react";
import { api } from "@/convex/_generated/api";

export default function RecentEvents() {
  const rows = useQuery(api.acp.recentRows);
  if (rows === undefined) return <div style={{ fontFamily: 'var(--font-geist-sans, system-ui)' }}>Loading…</div>;

  return (
    <div
      style={{
        marginTop: 12,
        flex: 1,
        minHeight: 0,
        overflow: 'auto',
        fontFamily: 'var(--font-geist-sans), system-ui, -apple-system, Segoe UI, Roboto, Ubuntu, Cantarell, Noto Sans, Helvetica Neue, Arial',
      }}
    >
      <table
        style={{
          width: '100%',
          borderCollapse: 'collapse',
          fontSize: 12,
          tableLayout: 'fixed',
        }}
      >
        <thead>
          <tr>
            <th style={{ textAlign: 'left', padding: 6, borderBottom: '1px solid #333', position: 'sticky', top: 0, background: 'rgba(0,0,0,0.6)' }}>Time</th>
            <th style={{ textAlign: 'left', padding: 6, borderBottom: '1px solid #333', position: 'sticky', top: 0, background: 'rgba(0,0,0,0.6)' }}>Event</th>
            <th style={{ textAlign: 'left', padding: 6, borderBottom: '1px solid #333', position: 'sticky', top: 0, background: 'rgba(0,0,0,0.6)' }}>Update / Message</th>
            <th style={{ textAlign: 'left', padding: 6, borderBottom: '1px solid #333', position: 'sticky', top: 0, background: 'rgba(0,0,0,0.6)' }}>ToolCall</th>
          </tr>
        </thead>
        <tbody>
          {(rows as any[]).map((r) => {
            const baseCell: React.CSSProperties = {
              padding: 6,
              borderBottom: '1px solid #222',
              whiteSpace: 'nowrap',
              overflow: 'hidden',
              textOverflow: 'ellipsis',
            };
            const msgCell: React.CSSProperties = {
              ...baseCell,
              whiteSpace: 'pre-wrap',
              overflow: 'hidden',
              textOverflow: 'clip',
              wordBreak: 'break-word',
            };
            return (
              <tr key={(r as any).id ?? (r as any)._id}>
                <td style={baseCell}>{new Date((r as any).ts as number).toLocaleString()}</td>
                <td style={baseCell}>{(r as any).event}</td>
                <td style={(r as any).event === 'agent_message' ? msgCell : baseCell}>
                  {(r as any).event === 'agent_message' ? (r as any).message : (r as any).update ?? ''}
                </td>
                <td style={baseCell}>{(r as any).toolCallId ?? ''}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

