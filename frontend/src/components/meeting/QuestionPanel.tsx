'use client';

import { TAG_ACTIVE_STYLE, TAG_BUTTON_LABEL, TAG_OPTIONS } from '@/lib/copilot/status';
import type { QuestionTag } from '@/lib/copilot/types';
import { useMeetingSession } from '@/lib/meeting/session';

/**
 * Left pane of a co-pilot meeting: the full selected question set, scrollable,
 * with manual tagging buttons (ASKED | NOT ASKED | ANSWERED) for each question.
 * User input only — no model or live detection runs during the call.
 */
export function QuestionPanel() {
  const { questions, tag, notes, phase } = useMeetingSession();
  const preview = phase === 'idle';
  const taggedCount = questions.filter((q) => q.user_tag !== null).length;

  const handleTagClick = (questionId: string, current: QuestionTag | null, clicked: QuestionTag) => {
    tag(questionId, current === clicked ? null : clicked);
  };

  return (
    <aside className="flex w-80 shrink-0 flex-col border-r border-gray-200 bg-white" aria-label="Questions">
      <div className="shrink-0 border-b border-gray-100 px-3 py-2">
        <div className="flex items-center justify-between">
          <h2 className="text-sm font-semibold text-gray-800">Questions</h2>
          <span className="text-xs text-gray-500">
            {questions.length === 0 ? 'none' : `${taggedCount}/${questions.length} tagged`}
          </span>
        </div>
        {!preview && questions.length > 0 && (
          <p className="mt-0.5 text-[11px] text-gray-400">
            Tap a tag as you cover each question. Tags stay editable until you generate the summary.
          </p>
        )}
        {preview && questions.length > 0 && (
          <p className="mt-0.5 text-[11px] text-gray-400">Preview. Press record to begin.</p>
        )}
      </div>
      <ol className="min-h-0 flex-1 space-y-2 overflow-y-auto p-3">
        {questions.length === 0 && (
          <li className="text-sm text-gray-400">Choose a question set to see its questions.</li>
        )}
        {questions.map((q, i) => (
          <li
            key={q.id}
            data-question
            data-tag={q.user_tag ?? 'untagged'}
            className="rounded border border-gray-200 p-2.5"
          >
            <p className="text-sm font-medium text-gray-900">
              {i + 1}. {q.main_question}
            </p>
            {q.sample_questions.length > 0 && (
              <p className="mt-0.5 text-xs text-gray-500">Also: {q.sample_questions.join(' · ')}</p>
            )}
            <div className="mt-2 flex flex-wrap gap-1" role="group" aria-label={`Tag question ${i + 1}`}>
              {TAG_OPTIONS.map((opt) => {
                const active = q.user_tag === opt;
                return (
                  <button
                    key={opt}
                    type="button"
                    data-tag-option={opt}
                    aria-pressed={active}
                    onClick={() => handleTagClick(q.id, q.user_tag, opt)}
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
            {!preview && (
              <textarea
                className="mt-1.5 w-full rounded border border-gray-300 p-1 text-xs"
                rows={1}
                placeholder="Notes"
                value={q.user_notes ?? ''}
                onChange={(e) => notes(q.id, e.target.value)}
              />
            )}
          </li>
        ))}
      </ol>
    </aside>
  );
}
