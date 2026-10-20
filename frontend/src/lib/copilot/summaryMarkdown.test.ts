import { describe, expect, it } from "vitest";
import { combinedSummaryMarkdown, defaultExportFilename, questionSummaryMarkdown } from "./summaryMarkdown";
import type { CopilotQuestion } from "./types";

const q = (over: Partial<CopilotQuestion>): CopilotQuestion => ({
  id: "q", meeting_id: "m", sort_order: 0, main_question: "Q?", sample_questions: [], sample_answers: [],
  status: "not_asked", rating: null, extracted_answer: null, missing_details: null, confidence: null,
  evidence_segment_ids: [], user_notes: null, manually_overridden: false, user_tag: null, tag_note: null, ...over,
});

describe("questionSummaryMarkdown", () => {
  it("lists every question with tag, status, reasoned answer and notes", () => {
    const md = questionSummaryMarkdown([
      q({ main_question: "Salary?", user_tag: "answered", status: "answered", extracted_answer: "Asked. The range discussed was 10,000-20,000.", missing_details: "bonus" }),
      q({ main_question: "Notice?", user_tag: "not_asked", status: "not_asked", extracted_answer: "Not asked in this call." }),
      q({ main_question: "Team\nsize?", tag_note: "Not explicitly tagged." }),
    ]);
    expect(md).toContain("## Part A — Question Summary");
    expect(md).toContain("**1. Salary?**\n- Tag: Answered\n- Status: Asked & Answered\n- Answer: Asked. The range discussed was 10,000-20,000.\n- Missing: bonus");
    expect(md).toContain("**2. Notice?**\n- Tag: Not asked\n- Status: Not Asked\n- Answer: Not asked in this call.");
    expect(md).toContain("**3. Team size?**\n- Tag: Untagged\n- Status: Not Asked\n- Answer: Not asked in this call.\n- Note: Not explicitly tagged.");
  });

  it("combines Part A and Part B in order and builds a date-stamped filename", () => {
    const combined = combinedSummaryMarkdown({
      title: "Sales Intake Call",
      createdAt: "2026-01-15T10:00:00Z",
      questions: [q({ main_question: "Budget?", user_tag: "asked", status: "answered", extracted_answer: "Asked. 10k-20k." })],
      meetingSummaryMarkdown: "### Overview\nGood call.",
    });
    const partAIdx = combined.indexOf("## Part A — Question Summary");
    const partBIdx = combined.indexOf("## Part B — Meeting Summary");
    expect(partAIdx).toBeGreaterThan(-1);
    expect(partBIdx).toBeGreaterThan(partAIdx);
    expect(defaultExportFilename("Sales Intake Call", "2026-01-15T10:00:00Z", "md")).toBe("Sales-Intake-Call-2026-01-15.md");
  });
});
