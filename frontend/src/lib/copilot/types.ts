// Intake Co-Pilot shared types (frontend mirror of the Rust model).

export type QuestionStatus = "not_asked" | "asked_not_answered" | "answered";

/** Manual tag tapped by the interviewer during the call (user input only). */
export type QuestionTag = "asked" | "not_asked" | "answered";

export type AnswerRating = "Poor" | "Partial" | "Strong";

export interface CopilotQuestion {
  id: string;
  meeting_id: string;
  sort_order: number;
  main_question: string;
  sample_questions: string[];
  sample_answers: string[];
  status: QuestionStatus;
  rating: AnswerRating | null;
  extracted_answer: string | null;
  missing_details: string | null;
  confidence: number | null;
  evidence_segment_ids: string[];
  user_notes: string | null;
  manually_overridden: boolean;
  user_tag: QuestionTag | null;
  tag_note: string | null;
}

export interface TranscriptSegment {
  id: string;
  meeting_id: string;
  seq: number;
  start_ms: number;
  end_ms: number;
  channel: number;
  speaker_label: string | null;
  text: string;
}

export interface CopilotReport {
  meeting_id: string;
  meeting_title: string;
  questions: CopilotQuestion[];
  action_points: string[];
  next_steps: string[];
  actions_discussed: string[];
  segments: TranscriptSegment[];
}
