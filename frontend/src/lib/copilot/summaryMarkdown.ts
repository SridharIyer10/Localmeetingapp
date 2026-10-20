import { STATUS_LABEL, TAG_LABEL } from "./status";
import type { CopilotQuestion } from "./types";

/** Render the reasoned answer line for one question in Part A. */
export function questionAnswerText(q: CopilotQuestion): string {
  const explicit = q.extracted_answer?.trim();
  if (explicit) return explicit;
  if (q.status === "not_asked") return "Not asked in this call.";
  if (q.status === "asked_not_answered") return "Asked, but no clear answer was given in the transcript.";
  return "—";
}

/** Part A — Question Summary as Markdown (copy / download). */
export function questionSummaryMarkdown(questions: CopilotQuestion[]): string {
  const lines: string[] = ["## Part A — Question Summary", ""];
  questions.forEach((q, i) => {
    const tagLabel = q.user_tag ? TAG_LABEL[q.user_tag] : "Untagged";
    lines.push(`**${i + 1}. ${q.main_question.replace(/\s+/g, " ").trim()}**`);
    lines.push(`- Tag: ${tagLabel}`);
    lines.push(`- Status: ${STATUS_LABEL[q.status]}`);
    lines.push(`- Answer: ${questionAnswerText(q)}`);
    if (q.tag_note?.trim()) lines.push(`- Note: ${q.tag_note.trim()}`);
    if (q.missing_details?.trim()) lines.push(`- Missing: ${q.missing_details.trim()}`);
    if (q.user_notes?.trim()) lines.push(`- Interviewer notes: ${q.user_notes.trim()}`);
    lines.push("");
  });
  return lines.join("\n").trimEnd() + "\n";
}

/** Combine Part A (when co-pilot questions exist) and Part B into one downloadable Markdown file. */
export function combinedSummaryMarkdown(opts: {
  title: string;
  createdAt?: string;
  questions?: CopilotQuestion[];
  meetingSummaryMarkdown?: string;
}): string {
  const parts: string[] = [`# ${opts.title.trim() || "Meeting"}`];
  if (opts.createdAt) {
    const d = new Date(opts.createdAt);
    if (!Number.isNaN(d.getTime())) {
      parts.push(`Date: ${d.toISOString().slice(0, 10)}`);
    }
  }
  parts.push("");
  if (opts.questions && opts.questions.length > 0) {
    parts.push(questionSummaryMarkdown(opts.questions).trimEnd());
    parts.push("");
  }
  if (opts.meetingSummaryMarkdown?.trim()) {
    if (opts.questions && opts.questions.length > 0) {
      parts.push("## Part B — Meeting Summary");
      parts.push("");
    }
    parts.push(opts.meetingSummaryMarkdown.trim());
    parts.push("");
  }
  return parts.join("\n").trimEnd() + "\n";
}

/** Default local filename: `<meeting title>-<YYYY-MM-DD>.<ext>`. */
export function defaultExportFilename(title: string | undefined, createdAt: string | undefined, ext: string): string {
  const base = (title ?? "meeting")
    .trim()
    .replace(/[^a-zA-Z0-9-_ ]+/g, "")
    .replace(/\s+/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "") || "meeting";
  let datePart = new Date().toISOString().slice(0, 10);
  if (createdAt) {
    const parsed = new Date(createdAt);
    if (!Number.isNaN(parsed.getTime())) {
      datePart = parsed.toISOString().slice(0, 10);
    }
  }
  const cleanExt = ext.replace(/^\./, "");
  return `${base}-${datePart}.${cleanExt}`;
}
