'use client';

import { useCallback, useEffect, useState } from 'react';
import { Pencil, Plus, Trash2 } from 'lucide-react';
import {
  deleteSummaryType, listSummaryTypes, saveSummaryType, summaryTypeSections, type SummarySection,
  type SummaryType,
} from '@/lib/copilot/api';

const msg = (e: unknown) => (e instanceof Error ? e.message : String(e));

interface Draft {
  id: string | null;
  name: string;
  prompt: string;
}

/** List every summary type; add, edit and delete custom ones (a name and a prompt). */
export default function SummaryTypesSettings() {
  const [types, setTypes] = useState<SummaryType[]>([]);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [shown, setShown] = useState<string | null>(null);
  const [sections, setSections] = useState<Record<string, SummarySection[]>>({});
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const reload = useCallback(async () => {
    try {
      setTypes(await listSummaryTypes());
    } catch (e) {
      setError(msg(e));
    }
  }, []);
  useEffect(() => {
    void reload();
  }, [reload]);

  async function save() {
    if (!draft) return;
    setBusy(true);
    setError(null);
    try {
      await saveSummaryType(draft.id, draft.name, draft.prompt);
      setDraft(null);
      await reload();
    } catch (e) {
      setError(msg(e));
    } finally {
      setBusy(false);
    }
  }

  async function toggleSections(id: string) {
    if (shown === id) return setShown(null);
    setShown(id);
    if (!sections[id]) {
      try {
        const d = await summaryTypeSections(id);
        setSections((p) => ({ ...p, [id]: d.section_details }));
      } catch (e) {
        setError(msg(e));
      }
    }
  }

  return (
    <div className="space-y-3">
      {error && <div role="alert" className="rounded border border-red-300 bg-red-50 p-3 text-sm text-red-800">{error}</div>}

      <ul className="divide-y divide-gray-100 rounded border border-gray-200 bg-white">
        {types.map((t) => (
          <li key={t.id} className="p-3" data-summary-type={t.id}>
            <div className="flex items-start gap-3">
              <div className="flex-1">
                <p className="text-sm font-medium text-gray-900">
                  {t.name}{' '}
                  <span className={`ml-1 rounded px-1.5 py-0.5 text-[10px] font-medium ${t.is_custom ? 'bg-purple-100 text-purple-800' : 'bg-gray-100 text-gray-600'}`}>
                    {t.is_custom ? 'Custom' : 'Built-in'}
                  </span>
                </p>
                <p className="text-xs text-gray-500">{t.is_custom ? (t.prompt ?? '').slice(0, 140) : t.description}</p>
              </div>
              {!t.is_custom && (
                <button type="button" className="text-xs text-blue-700" onClick={() => void toggleSections(t.id)}>
                  {shown === t.id ? 'Hide sections' : 'Show sections'}
                </button>
              )}
              {t.is_custom && (
                <>
                  <button type="button" aria-label={`Edit ${t.name}`} className="text-gray-400 hover:text-gray-700" onClick={() => setDraft({ id: t.id, name: t.name, prompt: t.prompt ?? '' })}>
                    <Pencil className="h-4 w-4" />
                  </button>
                  <button
                    type="button"
                    aria-label={`Delete ${t.name}`}
                    className="text-gray-400 hover:text-red-600"
                    onClick={() => {
                      if (window.confirm(`Delete the summary type "${t.name}"? Summaries already generated are kept.`)) {
                        void deleteSummaryType(t.id).then(reload).catch((e) => setError(msg(e)));
                      }
                    }}
                  >
                    <Trash2 className="h-4 w-4" />
                  </button>
                </>
              )}
            </div>
            {shown === t.id && sections[t.id] && (
              <ul className="mt-2 space-y-1 rounded bg-gray-50 p-2">
                {sections[t.id].map((s) => (
                  <li key={s.title} className="text-xs text-gray-700">
                    <span className="font-semibold">{s.title}</span>: {s.instruction}
                  </li>
                ))}
              </ul>
            )}
          </li>
        ))}
      </ul>

      {draft ? (
        <div className="space-y-2 rounded border border-gray-200 bg-white p-3" role="form" aria-label="Summary type">
          <input
            className="w-full rounded border border-gray-300 p-2 text-sm"
            placeholder="Name (e.g. Risk review)"
            aria-label="Summary type name"
            maxLength={80}
            value={draft.name}
            onChange={(e) => setDraft({ ...draft, name: e.target.value })}
          />
          <textarea
            className="min-h-[7rem] w-full rounded border border-gray-300 p-2 text-sm"
            placeholder="Prompt template: tell the model what the summary should contain and how to lay it out. Example: List every risk raised, who owns it, and the mitigation agreed."
            aria-label="Summary type prompt"
            maxLength={4000}
            value={draft.prompt}
            onChange={(e) => setDraft({ ...draft, prompt: e.target.value })}
          />
          <p className="text-xs text-gray-500">The meeting transcript is added for you. {draft.prompt.length}/4000</p>
          <div className="flex justify-end gap-2">
            <button type="button" className="rounded border border-gray-300 px-3 py-1 text-sm" onClick={() => setDraft(null)}>Cancel</button>
            <button type="button" disabled={busy || !draft.name.trim() || !draft.prompt.trim()} className="rounded bg-blue-600 px-3 py-1 text-sm text-white disabled:opacity-50" onClick={() => void save()}>
              {draft.id ? 'Save changes' : 'Add summary type'}
            </button>
          </div>
        </div>
      ) : (
        <button type="button" className="inline-flex items-center gap-1 rounded bg-blue-600 px-3 py-2 text-sm text-white" onClick={() => setDraft({ id: null, name: '', prompt: '' })}>
          <Plus className="h-4 w-4" /> New summary type
        </button>
      )}
    </div>
  );
}
