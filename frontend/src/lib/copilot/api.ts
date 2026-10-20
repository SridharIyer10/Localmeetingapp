// Typed wrappers over the Tauri commands used by co-pilot meetings, question sets
// and summary types. Every call requires the desktop shell.

import { invoke } from "@tauri-apps/api/core";
import type { CopilotQuestion, QuestionStatus, QuestionTag, TranscriptSegment } from "./types";

export function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export interface QuestionInput {
  main_question: string;
  sample_questions: string[];
  sample_answers: string[];
}

// ---- question sets (named, reusable) ----------------------------------------

export interface QuestionSet {
  id: string;
  name: string;
  created_at: string;
  updated_at: string;
}

/** One reusable question inside a set. */
export interface SetQuestion {
  id: string;
  bank_id: string | null;
  question: string;
  sample_questions_json: string;
  sample_answers_json: string;
}

function jsonList(text: string): string[] {
  try {
    const v: unknown = JSON.parse(text);
    return Array.isArray(v) ? v.map(String) : [];
  } catch {
    return [];
  }
}
export const sampleQuestionsOf = (q: SetQuestion) => jsonList(q.sample_questions_json);
export const sampleAnswersOf = (q: SetQuestion) => jsonList(q.sample_answers_json);

export const listSets = () => invoke<QuestionSet[]>("question_bank_group_list");
export const createSet = (name: string) => invoke<string>("question_bank_group_create", { name });
export const renameSet = (id: string, name: string) => invoke<void>("question_bank_group_rename", { id, name });
export const deleteSet = (id: string) => invoke<void>("question_bank_group_delete", { id });
export const listSetQuestions = () => invoke<SetQuestion[]>("question_bank_list");
export const addSetQuestion = (
  question: string,
  sampleQuestions: string[],
  sampleAnswers: string[],
  setId: string,
) => invoke<string>("question_bank_add", { question, sampleQuestions, sampleAnswers, bankId: setId });
export const deleteSetQuestion = (id: string) => invoke<void>("question_bank_delete", { id });

export interface SetImportResult {
  id: string;
  /** true: new set created; false: an existing set's questions were replaced. */
  created: boolean;
  count: number;
}
export const importSet = (name: string, questions: QuestionInput[]) =>
  invoke<SetImportResult>("question_set_import", { name, questions });

/** Native "save as" dialog + write. Resolves to the saved path, or null if cancelled. */
export const saveTextFile = (suggestedName: string, content: string) =>
  invoke<string | null>("save_text_file", { suggestedName, content });

// ---- a meeting's questions --------------------------------------------------

/** Create the draft meeting holding a copy of the set's questions; returns them. */
export const beginSession = (draftId: string, setId: string) =>
  invoke<CopilotQuestion[]>("copilot_begin_session", { draftId, setId });
export const listQuestions = (meetingId: string) =>
  invoke<CopilotQuestion[]>("copilot_list_questions", { meetingId });
export const listSegments = (meetingId: string) =>
  invoke<TranscriptSegment[]>("copilot_list_segments", { meetingId });
export const setQuestionTag = (questionId: string, tag: QuestionTag | null) =>
  invoke<void>("copilot_set_question_tag", { questionId, tag });
export const setQuestionNotes = (questionId: string, notes: string) =>
  invoke<void>("copilot_set_question_notes", { questionId, notes });
export const overrideVerdict = (
  questionId: string,
  status: QuestionStatus,
  rating: string | null,
  extractedAnswer: string | null,
) => invoke<void>("copilot_override_verdict", { questionId, status, rating, extractedAnswer });

/** Attach the saved recording (its own meeting row) to the co-pilot meeting's questions. */
export const finalizeRecording = (copilotMeetingId: string, savedMeetingId: string) =>
  invoke<number>("copilot_finalize_recording", { copilotMeetingId, savedMeetingId });

// ---- post-meeting question summary (Part A) ---------------------------------

export interface AnalysisReport {
  questions: number;
  engine: string;
  note: string | null;
  model_calls: number;
}
export interface AnalysisStatus {
  stage: "indexing" | "evaluating_questions" | "extracting_notes" | "complete" | "failed" | null;
  engine: string | null;
}
export const runAnalysis = (meetingId: string) => invoke<AnalysisReport>("copilot_run_analysis", { meetingId });
export const analysisStatus = (meetingId: string) => invoke<AnalysisStatus>("copilot_analysis_status", { meetingId });

// ---- summary types ----------------------------------------------------------

export interface SummaryType {
  id: string;
  name: string;
  description: string;
  is_custom: boolean;
  /** The user's prompt (custom types only). */
  prompt: string | null;
}
export const listSummaryTypes = () => invoke<SummaryType[]>("api_list_templates");
export const saveSummaryType = (id: string | null, name: string, prompt: string) =>
  invoke<string>("api_save_custom_template", { id, name, prompt });
export const deleteSummaryType = (id: string) => invoke<void>("api_delete_custom_template", { id });
export interface SummarySection {
  title: string;
  instruction: string;
}
export const summaryTypeSections = (templateId: string) =>
  invoke<{ sections: string[]; section_details: SummarySection[] }>("api_get_template_details", { templateId });
