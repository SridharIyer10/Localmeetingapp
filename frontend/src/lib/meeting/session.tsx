'use client';

// MeetingSession: the one place that knows what kind of meeting is running.
// Standard and co-pilot meetings use the same recording service, transcript
// pipeline, shell and phases; the only differences are data: a co-pilot session
// also carries a question set, its questions and their manual interviewer tags.
// No model or live evaluation runs during the call — all compute goes to transcription.

import {
  createContext, useCallback, useContext, useMemo, useRef, useState, type ReactNode,
} from 'react';
import { toast } from 'sonner';
import { useRecordingState, RecordingStatus } from '@/contexts/RecordingStateContext';
import {
  beginSession, finalizeRecording, listSetQuestions, sampleAnswersOf,
  sampleQuestionsOf, setQuestionNotes, setQuestionTag,
} from '@/lib/copilot/api';
import type { CopilotQuestion, QuestionTag } from '@/lib/copilot/types';
import { derivePhase, type MeetingKind, type MeetingPhase, type RecordingStatusValue } from './phase';

interface MeetingSession {
  kind: MeetingKind | null;
  phase: MeetingPhase;
  setKind: (k: MeetingKind | null) => void;
  /** Co-pilot only. */
  setId: string | null;
  /** Select a question set and load its questions into the session. */
  chooseSet: (id: string | null) => Promise<void>;
  questions: CopilotQuestion[];
  /** Set or toggle a manual tag (ASKED | NOT ASKED | ANSWERED) on a question. */
  tag: (id: string, next: QuestionTag | null) => void;
  notes: (id: string, text: string) => void;
  /** Runs before the recorder starts. Throws if the session cannot start. */
  prepare: () => Promise<void>;
  /** Runs after the recording is saved as `savedMeetingId`. Never throws. */
  complete: (savedMeetingId: string) => Promise<void>;
  reset: () => void;
}

const Ctx = createContext<MeetingSession | null>(null);

export function useMeetingSession(): MeetingSession {
  const v = useContext(Ctx);
  if (!v) throw new Error('useMeetingSession must be used within MeetingSessionProvider');
  return v;
}

export function MeetingSessionProvider({ children }: { children: ReactNode }) {
  const rec = useRecordingState();

  const [kind, setKindState] = useState<MeetingKind | null>(null);
  const [setId, setSetId] = useState<string | null>(null);
  const [questions, setQuestions] = useState<CopilotQuestion[]>([]);
  // The draft id, kind, and latest questions are also read from async callbacks.
  const draftId = useRef<string | null>(null);
  const kindRef = useRef<MeetingKind | null>(null);
  const questionsRef = useRef<CopilotQuestion[]>([]);
  kindRef.current = kind;
  questionsRef.current = questions;

  const phase = derivePhase({
    status: rec.status as RecordingStatusValue,
    isRecording: rec.isRecording,
    isPaused: rec.isPaused,
  });

  const reset = useCallback(() => {
    setKindState(null);
    setSetId(null);
    setQuestions([]);
    draftId.current = null;
  }, []);

  const setKind = useCallback((k: MeetingKind | null) => {
    setKindState(k);
    setSetId(null);
    setQuestions([]);
    draftId.current = null;
  }, []);

  const chooseSet = useCallback(async (id: string | null) => {
    setSetId(id);
    draftId.current = null;
    if (!id) {
      setQuestions([]);
      return;
    }
    try {
      const items = (await listSetQuestions()).filter((i) => i.bank_id === id);
      setQuestions(
        items.map((i, n) => ({
          id: `preview-${i.id}`,
          meeting_id: '',
          sort_order: n,
          main_question: i.question,
          sample_questions: sampleQuestionsOf(i),
          sample_answers: sampleAnswersOf(i),
          status: 'not_asked',
          rating: null,
          extracted_answer: null,
          missing_details: null,
          confidence: null,
          evidence_segment_ids: [],
          user_notes: null,
          manually_overridden: false,
          user_tag: null,
          tag_note: null,
        })),
      );
    } catch (e) {
      toast.error('Could not load the question set', { description: e instanceof Error ? e.message : String(e) });
    }
  }, []);

  const prepare = useCallback(async () => {
    if (kindRef.current !== 'copilot') return;
    if (!setId) throw new Error('Choose a question set first.');
    // Preserve any tags or notes the interviewer already entered before pressing Record.
    const priorByOrder = new Map(
      questionsRef.current.map((q) => [q.sort_order, { tag: q.user_tag, notes: q.user_notes }]),
    );
    draftId.current ??= `draft-${crypto.randomUUID()}`;
    const created = await beginSession(draftId.current, setId);
    const merged = await Promise.all(
      created.map(async (q) => {
        const prior = priorByOrder.get(q.sort_order);
        let user_tag = q.user_tag;
        let user_notes = q.user_notes;
        if (prior?.tag) {
          user_tag = prior.tag;
          await setQuestionTag(q.id, prior.tag).catch(() => undefined);
        }
        if (prior?.notes) {
          user_notes = prior.notes;
          await setQuestionNotes(q.id, prior.notes).catch(() => undefined);
        }
        return { ...q, user_tag, user_notes };
      }),
    );
    setQuestions(merged);
  }, [setId]);

  const complete = useCallback(async (savedMeetingId: string) => {
    const draft = draftId.current;
    if (kindRef.current !== 'copilot' || !draft) return;
    try {
      // Re-key the session's questions and their manual tags onto the saved meeting.
      // No post-call model runs here: summary generation only runs when the user
      // clicks "Generate Summary" on the meeting details screen.
      await finalizeRecording(draft, savedMeetingId);
    } catch (e) {
      toast.error('Could not attach the questions to this meeting', {
        description: e instanceof Error ? e.message : String(e),
      });
    } finally {
      draftId.current = null;
    }
  }, []);

  const tag = useCallback((id: string, next: QuestionTag | null) => {
    setQuestions((prev) => prev.map((q) => (q.id === id ? { ...q, user_tag: next } : q)));
    if (!id.startsWith('preview-')) {
      void setQuestionTag(id, next).catch((e) =>
        toast.error('Could not save tag', { description: e instanceof Error ? e.message : String(e) }),
      );
    }
  }, []);

  const notes = useCallback((id: string, text: string) => {
    setQuestions((prev) => prev.map((q) => (q.id === id ? { ...q, user_notes: text } : q)));
    if (!id.startsWith('preview-')) {
      void setQuestionNotes(id, text).catch(() => undefined);
    }
  }, []);

  const value = useMemo<MeetingSession>(
    () => ({
      kind, phase, setKind, setId, chooseSet, questions, tag, notes, prepare, complete, reset,
    }),
    [kind, phase, setKind, setId, chooseSet, questions, tag, notes, prepare, complete, reset],
  );
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

// Re-exported so consumers need one import for the recording status enum values.
export { RecordingStatus };
