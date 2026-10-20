import type { QuestionStatus, QuestionTag } from "./types";

/** The post-call status vocabulary, used by the question summary. */
export const STATUS_LABEL: Record<QuestionStatus, string> = {
  answered: "Asked & Answered",
  asked_not_answered: "Asked, Not Answered",
  not_asked: "Not Asked",
};

export const STATUS_STYLE: Record<QuestionStatus, string> = {
  answered: "bg-green-100 text-green-800 border-green-300",
  asked_not_answered: "bg-amber-100 text-amber-800 border-amber-300",
  not_asked: "bg-gray-100 text-gray-600 border-gray-300",
};

/** The three manual tag buttons shown during the call (in display order). */
export const TAG_OPTIONS: readonly QuestionTag[] = ["asked", "not_asked", "answered"] as const;

export const TAG_BUTTON_LABEL: Record<QuestionTag, string> = {
  asked: "ASKED",
  not_asked: "NOT ASKED",
  answered: "ANSWERED",
};

export const TAG_LABEL: Record<QuestionTag, string> = {
  asked: "Asked",
  not_asked: "Not asked",
  answered: "Answered",
};

export const TAG_ACTIVE_STYLE: Record<QuestionTag, string> = {
  asked: "bg-amber-600 text-white border-amber-600",
  not_asked: "bg-gray-700 text-white border-gray-700",
  answered: "bg-green-600 text-white border-green-600",
};
