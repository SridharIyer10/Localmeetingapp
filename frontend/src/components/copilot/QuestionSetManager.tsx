'use client';

import { useCallback, useEffect, useState } from 'react';
import { ChevronDown, ChevronRight, Pencil, Plus, Trash2 } from 'lucide-react';
import { CsvActions } from './CsvActions';
import {
  addSetQuestion, createSet, deleteSet, deleteSetQuestion, listSetQuestions, listSets, renameSet,
  sampleAnswersOf, sampleQuestionsOf, type QuestionSet, type SetQuestion,
} from '@/lib/copilot/api';

const msg = (e: unknown) => (e instanceof Error ? e.message : String(e));
const pipes = (v: string) => v.split('|').map((s) => s.trim()).filter(Boolean);

/** Create, edit, export and import question sets. */
export default function QuestionSetManager() {
  const [sets, setSets] = useState<QuestionSet[]>([]);
  const [items, setItems] = useState<SetQuestion[]>([]);
  const [open, setOpen] = useState<Set<string>>(new Set());
  const [name, setName] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const reload = useCallback(async () => {
    try {
      const [s, i] = await Promise.all([listSets(), listSetQuestions()]);
      setSets(s);
      setItems(i);
    } catch (e) {
      setError(msg(e));
    }
  }, []);
  useEffect(() => {
    void reload();
  }, [reload]);

  async function run(action: () => Promise<unknown>) {
    setBusy(true);
    setError(null);
    try {
      await action();
      await reload();
    } catch (e) {
      setError(msg(e));
    } finally {
      setBusy(false);
    }
  }

  const expand = (id: string) => setOpen((p) => new Set(p).add(id));
  const toggle = (id: string) =>
    setOpen((p) => {
      const n = new Set(p);
      if (n.has(id)) n.delete(id);
      else n.add(id);
      return n;
    });

  return (
    <div className="space-y-4">
      <CsvActions onImported={(id) => { void reload(); expand(id); }} />

      <div className="flex flex-wrap items-center gap-2">
        <input
          className="min-w-[16rem] flex-1 rounded border border-gray-300 p-2 text-sm"
          placeholder="New question set name (e.g. Sales intake)"
          aria-label="New question set name"
          maxLength={120}
          value={name}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && name.trim()) void run(async () => { expand(await createSet(name)); setName(''); });
          }}
        />
        <button
          type="button"
          disabled={busy || !name.trim()}
          className="inline-flex items-center gap-1 rounded bg-blue-600 px-3 py-2 text-sm text-white disabled:opacity-50"
          onClick={() => void run(async () => { expand(await createSet(name)); setName(''); })}
        >
          <Plus className="h-4 w-4" /> Create question set
        </button>
      </div>

      {error && <div role="alert" className="rounded border border-red-300 bg-red-50 p-3 text-sm text-red-800">{error}</div>}

      {sets.length === 0 ? (
        <p className="rounded border border-dashed border-gray-300 p-6 text-center text-sm text-gray-500">
          No question sets yet. Create one above, or download the template, fill it in and import it.
        </p>
      ) : (
        sets.map((s) => (
          <SetCard
            key={s.id}
            set={s}
            items={items.filter((i) => i.bank_id === s.id)}
            expanded={open.has(s.id)}
            onToggle={() => toggle(s.id)}
            busy={busy}
            run={run}
            onError={setError}
            onImported={() => void reload()}
          />
        ))
      )}
    </div>
  );
}

function SetCard({
  set, items, expanded, onToggle, busy, run, onError, onImported,
}: {
  set: QuestionSet;
  items: SetQuestion[];
  expanded: boolean;
  onToggle: () => void;
  busy: boolean;
  run: (a: () => Promise<unknown>) => Promise<void>;
  onError: (m: string | null) => void;
  onImported: () => void;
}) {
  const [renaming, setRenaming] = useState(false);
  const [name, setName] = useState(set.name);
  const [adding, setAdding] = useState(false);
  const [q, setQ] = useState('');
  const [alts, setAlts] = useState('');
  const [answers, setAnswers] = useState('');

  return (
    <section className="rounded border border-gray-200 bg-white" aria-label={`Question set ${set.name}`}>
      <div className="flex items-center gap-2 p-3">
        <button type="button" onClick={onToggle} aria-expanded={expanded} aria-label="Toggle set">
          {expanded ? <ChevronDown className="h-4 w-4" /> : <ChevronRight className="h-4 w-4" />}
        </button>
        {renaming ? (
          <>
            <input className="flex-1 rounded border border-gray-300 p-1 text-sm" value={name} maxLength={120} aria-label="Set name" onChange={(e) => setName(e.target.value)} />
            <button type="button" className="text-sm text-blue-700" onClick={() => void run(async () => { await renameSet(set.id, name); setRenaming(false); })}>Save</button>
            <button type="button" className="text-sm text-gray-500" onClick={() => { setName(set.name); setRenaming(false); }}>Cancel</button>
          </>
        ) : (
          <>
            <h3 className="flex-1 text-base font-semibold text-gray-900">{set.name}</h3>
            <span className="text-xs text-gray-500">{items.length} question{items.length === 1 ? '' : 's'}</span>
            <button type="button" aria-label="Rename set" className="text-gray-400 hover:text-gray-700" onClick={() => setRenaming(true)}><Pencil className="h-4 w-4" /></button>
            <button
              type="button"
              aria-label="Delete set"
              className="text-gray-400 hover:text-red-600"
              onClick={() => {
                if (window.confirm(`Delete "${set.name}" and its ${items.length} question(s)? Past meetings keep their questions.`)) void run(() => deleteSet(set.id));
              }}
            >
              <Trash2 className="h-4 w-4" />
            </button>
          </>
        )}
      </div>

      {expanded && (
        <div className="space-y-3 border-t border-gray-100 p-3">
          <div className="flex flex-wrap items-start gap-2">
            <button type="button" onClick={() => setAdding((v) => !v)} className="inline-flex items-center gap-1 rounded border border-gray-300 px-3 py-1 text-xs hover:bg-gray-50">
              <Plus className="h-4 w-4" /> Add question
            </button>
            <CsvActions current={set} compact onImported={onImported} />
          </div>

          {adding && (
            <div className="space-y-2 rounded border border-gray-200 p-3">
              <input className="w-full rounded border border-gray-300 p-2 text-sm" placeholder="Question" value={q} onChange={(e) => setQ(e.target.value)} />
              <input className="w-full rounded border border-gray-300 p-2 text-sm" placeholder="Sample questions: other ways to ask it (separate with |)" value={alts} onChange={(e) => setAlts(e.target.value)} />
              <input className="w-full rounded border border-gray-300 p-2 text-sm" placeholder="Sample answers: what a good answer covers (separate with |)" value={answers} onChange={(e) => setAnswers(e.target.value)} />
              <div className="flex justify-end gap-2">
                <button type="button" onClick={() => setAdding(false)} className="rounded border border-gray-300 px-3 py-1 text-sm">Cancel</button>
                <button
                  type="button"
                  disabled={busy || !q.trim()}
                  className="rounded bg-blue-600 px-3 py-1 text-sm text-white disabled:opacity-50"
                  onClick={() => void run(async () => { await addSetQuestion(q, pipes(alts), pipes(answers), set.id); setQ(''); setAlts(''); setAnswers(''); setAdding(false); })}
                >
                  Add to {set.name}
                </button>
              </div>
            </div>
          )}

          {items.length === 0 ? (
            <p className="text-sm text-gray-400">No questions in this set yet.</p>
          ) : (
            <ol className="divide-y divide-gray-100">
              {items.map((b, i) => (
                <li key={b.id} className="flex items-start gap-3 py-2">
                  <span className="w-5 text-sm text-gray-400">{i + 1}.</span>
                  <div className="flex-1">
                    <p className="text-sm text-gray-900">{b.question}</p>
                    {sampleQuestionsOf(b).length > 0 && <p className="mt-0.5 text-xs text-gray-500">Also asked as: {sampleQuestionsOf(b).join(' · ')}</p>}
                    {sampleAnswersOf(b).length > 0 && <p className="mt-0.5 text-xs text-gray-500">Sample answers: {sampleAnswersOf(b).join(' · ')}</p>}
                  </div>
                  <button type="button" aria-label="Delete question" className="text-gray-400 hover:text-red-600" onClick={() => void run(() => deleteSetQuestion(b.id)).catch((e) => onError(msg(e)))}>
                    <Trash2 className="h-4 w-4" />
                  </button>
                </li>
              ))}
            </ol>
          )}
        </div>
      )}
    </section>
  );
}
