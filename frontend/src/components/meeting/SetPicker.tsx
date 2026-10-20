'use client';

import { useCallback, useEffect, useState } from 'react';
import Link from 'next/link';
import { CsvActions } from '@/components/copilot/CsvActions';
import { listSetQuestions, listSets, type QuestionSet } from '@/lib/copilot/api';
import { useMeetingSession } from '@/lib/meeting/session';

/** Dropdown of question sets for a co-pilot meeting. Locked once recording starts. */
export function SetPicker() {
  const { setId, chooseSet, phase } = useMeetingSession();
  const [sets, setSets] = useState<QuestionSet[]>([]);
  const [counts, setCounts] = useState<Record<string, number>>({});
  const [loaded, setLoaded] = useState(false);

  const load = useCallback(async () => {
    try {
      const [s, items] = await Promise.all([listSets(), listSetQuestions()]);
      setSets(s);
      const c: Record<string, number> = {};
      items.forEach((i) => {
        if (i.bank_id) c[i.bank_id] = (c[i.bank_id] ?? 0) + 1;
      });
      setCounts(c);
    } finally {
      setLoaded(true);
    }
  }, []);
  useEffect(() => {
    void load().catch(() => undefined);
  }, [load]);

  const idle = phase === 'idle';
  const current = sets.find((s) => s.id === setId) ?? null;

  return (
    <div className="flex flex-wrap items-start justify-end gap-3">
      {loaded && sets.length === 0 ? (
        <p className="text-sm text-gray-600">
          No question sets yet. Import a CSV here, or create one in{' '}
          <Link href="/settings" className="text-blue-700 underline">Settings → Summarization</Link>.
        </p>
      ) : (
        <label className="flex items-center gap-2 text-sm text-gray-700">
          <span className="font-medium">Question set</span>
          <select
            aria-label="Question set"
            className="min-w-[14rem] rounded border border-gray-300 bg-white p-1.5 text-sm disabled:bg-gray-100"
            value={setId ?? ''}
            disabled={!idle}
            onChange={(e) => void chooseSet(e.target.value || null)}
          >
            <option value="">Select a question set…</option>
            {sets.map((s) => (
              <option key={s.id} value={s.id} disabled={(counts[s.id] ?? 0) === 0}>
                {s.name} ({counts[s.id] ?? 0})
              </option>
            ))}
          </select>
        </label>
      )}
      {idle && (
        <CsvActions
          compact
          current={current}
          onImported={(id) => void load().then(() => chooseSet(id))}
        />
      )}
    </div>
  );
}
