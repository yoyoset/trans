import type { ReactNode } from "react";

export function Section({ title, children, actions }: { title: string; children: ReactNode; actions?: ReactNode }) {
  return (
    <section className="space-y-3 rounded-lg border border-zinc-800 bg-zinc-900/60 p-4">
      <header className="flex items-center justify-between gap-2">
        <h2 className="text-sm font-semibold text-zinc-200">{title}</h2>
        {actions}
      </header>
      {children}
    </section>
  );
}
