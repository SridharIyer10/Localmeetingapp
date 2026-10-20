'use client';

import { useRef, useState } from 'react';
import { Download, FileDown, Upload } from 'lucide-react';
import { toast } from 'sonner';
import { csvFilename, templateCsv, toCsv, type CsvRow } from '@/lib/copilot/csv';
import {
  importSet, listSetQuestions, listSets, sampleAnswersOf, sampleQuestionsOf, saveTextFile,
  type QuestionSet,
} from '@/lib/copilot/api';
import {
  groupBySet, ImportError, nameFromFilename, parseQuestionWorkbook, type ParsedSet, type RowError,
} from '@/lib/copilot/import';

const msg = (e: unknown) => (e instanceof Error ? e.message : String(e));

async function save(name: string, content: string) {
  const path = await saveTextFile(name, content);
  if (path) toast.success('Saved', { description: path });
}

/** Download the blank template (always) or one set's questions, in the import schema. */
export async function downloadTemplate() {
  try {
    await save('question-set-template.csv', templateCsv());
  } catch (e) {
    toast.error('Could not save the file', { description: msg(e) });
  }
}

export async function downloadSet(set: QuestionSet) {
  try {
    const rows: CsvRow[] = (await listSetQuestions())
      .filter((i) => i.bank_id === set.id)
      .map((i) => ({
        set: set.name,
        question: i.question,
        sample_questions: sampleQuestionsOf(i),
        sample_answers: sampleAnswersOf(i),
      }));
    await save(csvFilename(set.name), toCsv(rows));
  } catch (e) {
    toast.error('Could not save the file', { description: msg(e) });
  }
}

interface Preview {
  filename: string;
  sets: (ParsedSet & { existing: QuestionSet | undefined; existingCount: number })[];
  errors: RowError[];
  warnings: string[];
}

/**
 * Template download + CSV import with a preview. Importing a file whose
 * `question_set` already exists replaces that set's questions (after a
 * confirmation that says so); otherwise it creates a new set.
 */
export function CsvActions({
  current,
  onImported,
  compact = false,
}: {
  /** The set currently selected/being edited, if any. */
  current?: QuestionSet | null;
  onImported?: (setId: string) => void;
  compact?: boolean;
}) {
  const fileRef = useRef<HTMLInputElement>(null);
  const [preview, setPreview] = useState<Preview | null>(null);
  const [fatal, setFatal] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function onFile(file: File) {
    setFatal(null);
    try {
      const parsed = parseQuestionWorkbook(await file.arrayBuffer(), file.name);
      const [sets, items] = await Promise.all([listSets(), listSetQuestions()]);
      const grouped = groupBySet(parsed.questions, current?.name ?? nameFromFilename(file.name));
      setPreview({
        filename: file.name,
        errors: parsed.errors,
        warnings: parsed.warnings,
        sets: grouped.map((g) => {
          const existing = sets.find((s) => s.name.trim().toLowerCase() === g.name.trim().toLowerCase());
          return { ...g, existing, existingCount: existing ? items.filter((i) => i.bank_id === existing.id).length : 0 };
        }),
      });
    } catch (e) {
      setPreview(null);
      setFatal(e instanceof ImportError ? e.message : msg(e));
    }
  }

  async function confirm() {
    if (!preview) return;
    setBusy(true);
    try {
      let lastId = '';
      for (const s of preview.sets) {
        const r = await importSet(s.name, s.questions.map(({ main_question, sample_questions, sample_answers }) => ({ main_question, sample_questions, sample_answers })));
        lastId = r.id;
        toast.success(r.created ? `Created "${s.name}"` : `Updated "${s.name}"`, { description: `${r.count} question${r.count === 1 ? '' : 's'}` });
      }
      setPreview(null);
      onImported?.(lastId);
    } catch (e) {
      setFatal(msg(e));
    } finally {
      setBusy(false);
    }
  }

  const total = preview?.sets.reduce((n, s) => n + s.questions.length, 0) ?? 0;
  const btn = `inline-flex items-center gap-1 rounded border border-gray-300 bg-white px-3 ${compact ? 'py-1 text-xs' : 'py-1.5 text-sm'} hover:bg-gray-50`;

  return (
    <div className="space-y-2">
      <div className="flex flex-wrap gap-2">
        <button type="button" className={btn} onClick={() => void downloadTemplate()}>
          <FileDown className="h-4 w-4" /> Download template
        </button>
        {current && (
          <button type="button" className={btn} onClick={() => void downloadSet(current)}>
            <Download className="h-4 w-4" /> Download “{current.name}”
          </button>
        )}
        <button type="button" className={btn} onClick={() => fileRef.current?.click()}>
          <Upload className="h-4 w-4" /> Import CSV
        </button>
        <input
          ref={fileRef}
          type="file"
          accept=".csv,.xlsx,.xls"
          className="hidden"
          aria-label="Import question set file"
          onChange={(e) => {
            const f = e.target.files?.[0];
            if (f) void onFile(f);
            e.target.value = '';
          }}
        />
      </div>

      {fatal && <p role="alert" className="rounded border border-red-300 bg-red-50 p-2 text-sm text-red-800">{fatal}</p>}

      {preview && (
        <div role="dialog" aria-label="Import preview" className="space-y-3 rounded border border-blue-200 bg-blue-50 p-3 text-sm">
          <p className="font-medium text-blue-900">Import “{preview.filename}”</p>
          {preview.sets.length === 0 && <p className="text-blue-900">No valid questions found.</p>}
          <ul className="space-y-1">
            {preview.sets.map((s) => (
              <li key={s.name} className="text-blue-900">
                {s.existing ? (
                  <>
                    <b>Update “{s.existing.name}”</b>: its {s.existingCount} existing question{s.existingCount === 1 ? '' : 's'} will be
                    replaced by these {s.questions.length}.
                  </>
                ) : (
                  <>
                    <b>Create “{s.name}”</b> with {s.questions.length} question{s.questions.length === 1 ? '' : 's'}.
                  </>
                )}
              </li>
            ))}
          </ul>
          {preview.errors.length > 0 && (
            <div className="rounded border border-red-200 bg-white p-2">
              <p className="mb-1 font-medium text-red-800">
                {preview.errors.length} row{preview.errors.length === 1 ? '' : 's'} will be skipped:
              </p>
              <table className="w-full text-xs text-red-800">
                <tbody>
                  {preview.errors.map((e, i) => (
                    <tr key={i}>
                      <td className="w-16 pr-2 align-top font-mono">Row {e.row}</td>
                      <td>{e.message}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
          {preview.warnings.map((w) => (
            <p key={w} className="text-xs text-amber-800">{w}</p>
          ))}
          <div className="flex justify-end gap-2">
            <button type="button" className={btn} onClick={() => setPreview(null)}>Cancel</button>
            <button
              type="button"
              disabled={busy || total === 0}
              onClick={() => void confirm()}
              className="rounded bg-blue-600 px-3 py-1.5 text-sm text-white disabled:opacity-50"
            >
              Import {total} question{total === 1 ? '' : 's'}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
