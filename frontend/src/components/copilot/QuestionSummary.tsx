'use client';

import { useCallback, useEffect, useRef, useState } from 'react';
import { ChevronDown, ChevronRight, ClipboardCheck, Copy } from 'lucide-react';
import { toast } from 'sonner';
import { StatusBadge } from '@/components/meeting/StatusBadge';
import {
  analysisStatus, listQuestions, listSegments, overrideVerdict, runAnalysis, setQuestionTag,
  type AnalysisStatus,
} from '@/lib/copilot/api';
import { STATUS_LABEL, TAG_ACTIVE_STYLE, TAG_BUTTON_LABEL, TAG_LABEL, TAG_OPTIONS } from '@/lib/copilot/status';
import { questionAnswerText, questionSummaryMarkdown } from '@/lib/copilot/summaryMarkdown';
import type { CopilotQuestion, QuestionStatus, QuestionTag, TranscriptSegment } from '@/lib/copilot/types';

const RUNNING = ['indexing', 'evaluating_questions', 'extracting_notes'];
const STAGE_TEXT: Record<string, string> = {
  indexing: 'Reading the transcript…',
  evaluating_questions: 'Interpreting the transcript for each question…',
  extracting_notes: 'Finishing…',
};

/** null while loading; true once we know the meeting has a question set. */
export function useIsCopilotMeeting(meetingId: string): boolean | null {
  const [v, setV] = useState<boolean | null>(null);
  useEffect(() => {
    let cancelled = false;
    setV(null);
    listQuestions(meetingId)
      .then((qs) => !cancelled && setV(qs.length > 0))
      .catch(() => !cancelled && setV(false));
    return () => {
      cancelled = true;
    };
  }, [meetingId]);
  return v;
}

const seqOf = (id: string) => (/^t(\d+)$/.exec(id) ? Number(id.slice(1)) : -1);
const msg = (e: unknown) => (e instanceof Error ? e.message : String(e));

/**
 * Part A — Question Summary of a co-pilot meeting.
 *
 * Before "Generate Summary" is clicked (`stage === null`), questions and their
 * manual tags (ASKED | NOT ASKED | ANSWERED) remain editable. When the user
 * clicks "Generate Summary", `generationTrigger` increments, locking the tags
 * and running the tag-aware question analysis alongside Part B (Meeting Summary).
 */
export default function QuestionSummary({
  meetingId,
  generationTrigger = 0,
  onQuestionsChange,
}: {
  meetingId: string;
  generationTrigger?: number;
  onQuestionsChange?: (questions: CopilotQuestion[]) => void;
}) {
  const [questions, setQuestions] = useState<CopilotQuestion[] | null>(null);
  const [segments, setSegments] = useState<TranscriptSegment[]>([]);
  const [status, setStatus] = useState<AnalysisStatus>({ stage: null, engine: null });
  const [open, setOpen] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const lastTriggerRef = useRef(generationTrigger);

  const updateQuestions = useCallback(
    (next: CopilotQuestion[] | ((prev: CopilotQuestion[]) => CopilotQuestion[])) => {
      setQuestions((prev) => {
        const resolved = typeof next === 'function' ? next(prev ?? []) : next;
        onQuestionsChange?.(resolved);
        return resolved;
      });
    },
    [onQuestionsChange],
  );

  const load = useCallback(async () => {
    try {
      const [qs, st] = await Promise.all([listQuestions(meetingId), analysisStatus(meetingId)]);
      updateQuestions(qs);
      setStatus(st);
      if (qs.length > 0) setSegments(await listSegments(meetingId));
    } catch (e) {
      setError(msg(e));
      setQuestions((prev) => prev ?? []);
    }
  }, [meetingId, updateQuestions]);

  useEffect(() => {
    void load();
  }, [load]);

  const runQuestionAnalysis = useCallback(async () => {
    setError(null);
    setStatus({ stage: 'indexing', engine: null });
    try {
      const report = await runAnalysis(meetingId);
      if (report.note) toast.message(report.note);
    } catch (e) {
      setError(msg(e));
    }
    await load();
  }, [meetingId, load]);

  // Triggered when the user clicks "Generate Summary" or "Regenerate Summary".
  useEffect(() => {
    if (generationTrigger > lastTriggerRef.current) {
      lastTriggerRef.current = generationTrigger;
      if (questions && questions.length > 0) {
        void runQuestionAnalysis();
      }
    }
  }, [generationTrigger, questions, runQuestionAnalysis]);

  // Poll while analysis is in flight.
  const running = status.stage !== null && RUNNING.includes(status.stage);
  useEffect(() => {
    if (!running) return;
    const t = setInterval(() => void load(), 1500);
    return () => clearInterval(t);
  }, [running, load]);

  const tagsLocked = status.stage !== null && status.stage !== 'failed';

  const handleTagEdit = useCallback(
    (q: CopilotQuestion, clicked: QuestionTag) => {
      if (tagsLocked) return;
      const next: QuestionTag | null = q.user_tag === clicked ? null : clicked;
      updateQuestions((prev) => prev.map((x) => (x.id === q.id ? { ...x, user_tag: next } : x)));
      void setQuestionTag(q.id, next).catch((e) => setError(msg(e)));
    },
    [tagsLocked, updateQuestions],
  );

  const correct = useCallback(
    (q: CopilotQuestion, next: QuestionStatus, rating: string | null) => {
      const r = next === 'answered' ? (rating ?? q.rating ?? 'Partial') : null;
      updateQuestions((prev) =>
        prev.map((x) =>
          x.id === q.id
            ? { ...x, status: next, rating: r as CopilotQuestion['rating'], manually_overridden: true }
            : x,
        ),
      );
      void overrideVerdict(q.id, next, r, q.extracted_answer).catch((e) => setError(msg(e)));
    },
    [updateQuestions],
  );

  if (!questions || questions.length === 0) return null;

  const answered = questions.filter((q) => q.status === 'answered').length;
  const taggedCount = questions.filter((q) => q.user_tag !== null).length;
  const notRun = status.stage === null;
  const failed = status.stage === 'failed';
  const completed = status.stage === 'complete';
  const rules = status.engine === 'rules';

  return (
    <section className="shrink-0 border-b border-gray-200 bg-white" aria-label="Part A — Question Summary">
      <div className="flex items-center gap-2 px-4 py-2.5">
        <button
          type="button"
          onClick={() => setOpen((v) => !v)}
          aria-expanded={open}
          className="flex flex-1 items-center gap-2 text-left text-sm font-semibold text-gray-800"
        >
          {open ? <ChevronDown className="h-4 w-4" /> : <ChevronRight className="h-4 w-4" />}
          <ClipboardCheck className="h-4 w-4 text-blue-600" />
          Part A — Question Summary
          <span className="font-normal text-gray-500">
            {running
              ? STAGE_TEXT[status.stage!]
              : completed
                ? `${answered}/${questions.length} answered`
                : `${taggedCount}/${questions.length} tagged`}
          </span>
        </button>
        {completed && !running && (
          <button
            type="button"
            title="Copy question summary"
            className="rounded p-1 text-gray-500 hover:bg-gray-100"
            onClick={() =>
              void navigator.clipboard
                .writeText(questionSummaryMarkdown(questions))
                .then(() => toast.success('Question summary copied'))
            }
          >
            <Copy className="h-4 w-4" />
          </button>
        )}
      </div>

      {open && (
        <div className="space-y-3 px-4 pb-4">
          {error && (
            <p role="alert" className="text-xs text-red-700">
              {error}
            </p>
          )}
          {running && (
            <p className="text-xs text-gray-500">
              Analysing the transcript using your question tags. This can take a moment.
            </p>
          )}
          {(notRun || failed) && !running && (
            <p className="text-xs text-gray-500">
              {failed
                ? 'The question summary did not finish. You can adjust tags below and click Generate Summary again.'
                : 'Tags remain editable until you click Generate Summary. Click Generate Summary to produce Part A (Question Summary) and Part B (Meeting Summary).'}
            </p>
          )}
          {completed && status.engine && (
            <p className="text-xs text-gray-500">
              {rules
                ? 'Answers were extracted by simple rules because no summary model was available, so they may be rough. Check them below.'
                : `Answers interpreted by ${status.engine}. Check them below and correct anything that is wrong.`}
            </p>
          )}

          {questions.map((q, i) => {
            const evidence = q.evidence_segment_ids
              .map((id) => segments.find((s) => s.seq === seqOf(id)))
              .filter((s): s is TranscriptSegment => !!s);
            const tagLabel = q.user_tag ? TAG_LABEL[q.user_tag] : 'Untagged';
            return (
              <div key={q.id} data-question data-tag={q.user_tag ?? 'untagged'} className="rounded border border-gray-200 p-3">
                <div className="flex items-start justify-between gap-3">
                  <p className="text-sm font-medium text-gray-900">
                    {i + 1}. {q.main_question}
                  </p>
                  {completed && <StatusBadge status={q.status} />}
                </div>

                {/* Before summary generation: tags remain editable. After: shown as locked tag badge. */}
                {!tagsLocked ? (
                  <div className="mt-2 flex flex-wrap items-center gap-1.5" role="group" aria-label={`Tag question ${i + 1}`}>
                    <span className="mr-1 text-xs text-gray-500">Tag:</span>
                    {TAG_OPTIONS.map((opt) => {
                      const active = q.user_tag === opt;
                      return (
                        <button
                          key={opt}
                          type="button"
                          data-tag-option={opt}
                          aria-pressed={active}
                          onClick={() => handleTagEdit(q, opt)}
                          className={`rounded border px-2 py-0.5 text-[11px] font-semibold transition-colors ${
                            active
                              ? TAG_ACTIVE_STYLE[opt]
                              : 'border-gray-300 bg-white text-gray-700 hover:bg-gray-50'
                          }`}
                        >
                          {TAG_BUTTON_LABEL[opt]}
                        </button>
                      );
                    })}
                  </div>
                ) : (
                  <p className="mt-1 text-xs text-gray-500">
                    Tag: <span className="font-medium text-gray-700">{tagLabel}</span>
                  </p>
                )}

                {completed && (
                  <p className="mt-1.5 text-sm text-gray-800" data-question-answer>
                    <span className="text-gray-500">Answer: </span>
                    {questionAnswerText(q)}
                  </p>
                )}

                {q.tag_note && (
                  <p className="mt-1 rounded bg-amber-50 px-2 py-1 text-xs text-amber-800" data-tag-note>
                    {q.tag_note}
                  </p>
                )}

                {q.missing_details && (
                  <p className="mt-0.5 text-sm text-gray-800">
                    <span className="text-gray-500">Missing: </span>
                    {q.missing_details}
                  </p>
                )}
                {q.user_notes && (
                  <p className="mt-0.5 text-sm text-gray-800">
                    <span className="text-gray-500">Notes: </span>
                    {q.user_notes}
                  </p>
                )}
                {evidence.length > 0 && (
                  <ul className="mt-1.5 space-y-0.5 border-l-2 border-blue-200 pl-2 text-xs text-gray-600">
                    {evidence.map((s) => (
                      <li key={s.id}>
                        <span className="text-gray-400">
                          [{Math.floor(s.start_ms / 60000)}:
                          {String(Math.floor(s.start_ms / 1000) % 60).padStart(2, '0')}]
                        </span>{' '}
                        {s.text}
                      </li>
                    ))}
                  </ul>
                )}
                {completed && (
                  <div className="mt-2 flex flex-wrap items-center gap-2 text-xs text-gray-600">
                    <label htmlFor={`verdict-${q.id}`}>Correct this:</label>
                    <select
                      id={`verdict-${q.id}`}
                      className="rounded border border-gray-300 p-1"
                      value={q.status}
                      onChange={(e) => correct(q, e.target.value as QuestionStatus, q.rating)}
                    >
                      {(Object.keys(STATUS_LABEL) as QuestionStatus[]).map((s) => (
                        <option key={s} value={s}>
                          {STATUS_LABEL[s]}
                        </option>
                      ))}
                    </select>
                    <select
                      aria-label="Answer quality"
                      className="rounded border border-gray-300 p-1 disabled:opacity-50"
                      disabled={q.status !== 'answered'}
                      value={q.rating ?? ''}
                      onChange={(e) => correct(q, 'answered', e.target.value || null)}
                    >
                      <option value="">No quality rating</option>
                      <option value="Poor">Poor</option>
                      <option value="Partial">Partial</option>
                      <option value="Strong">Strong</option>
                    </select>
                    {q.manually_overridden && <span className="text-amber-700">corrected</span>}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </section>
  );
}
