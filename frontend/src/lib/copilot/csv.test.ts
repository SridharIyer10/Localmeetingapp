import { describe, expect, it } from "vitest";
import { groupBySet, nameFromFilename, parseQuestionWorkbook } from "./import";
import { csvFilename, templateCsv, toCsv, type CsvRow } from "./csv";

const buf = (text: string) => new TextEncoder().encode(text).buffer as ArrayBuffer;

describe("question-set CSV", () => {
  it("round-trips commas, quotes, newlines, pipes-as-delimiters and unicode", () => {
    const rows: CsvRow[] = [
      {
        set: "HR, screening",
        question: 'Do you hold the "L2" visa,\nor equivalent?',
        sample_questions: ["Visa status?", "Work permit — held?"],
        sample_answers: ["Yes, L2", "Équivalent"],
      },
      { set: "HR, screening", question: "Notice period?", sample_questions: [], sample_answers: [] },
    ];
    const parsed = parseQuestionWorkbook(buf(toCsv(rows)), "x.csv");
    expect(parsed.errors).toEqual([]);
    expect(parsed.questions).toHaveLength(2);
    expect(parsed.questions[0]).toEqual({
      question_set: "HR, screening",
      main_question: 'Do you hold the "L2" visa,\nor equivalent?',
      sample_questions: ["Visa status?", "Work permit — held?"],
      sample_answers: ["Yes, L2", "Équivalent"],
    });
    expect(groupBySet(parsed.questions, "fallback")).toHaveLength(1);
  });

  it("neutralises spreadsheet formulas on export and restores them on import", () => {
    const csv = toCsv([{ set: "S", question: "=SUM(A1)", sample_questions: ["+1 call?"], sample_answers: ["@home"] }]);
    expect(csv).toContain("'=SUM(A1)");
    expect(csv).not.toMatch(/(^|,)=SUM/m);
    const back = parseQuestionWorkbook(buf(csv), "x.csv").questions[0];
    expect(back.main_question).toBe("=SUM(A1)");
    expect(back.sample_questions).toEqual(["+1 call?"]);
    expect(back.sample_answers).toEqual(["@home"]);
  });

  it("the downloadable template itself imports cleanly", () => {
    const parsed = parseQuestionWorkbook(buf(templateCsv()), "template.csv");
    expect(parsed.errors).toEqual([]);
    expect(groupBySet(parsed.questions, "x").map((s) => [s.name, s.questions.length])).toEqual([["Sales intake", 2]]);
  });

  it("reports row-level errors with spreadsheet line numbers", () => {
    const csv =
      "question_set,question,sample_questions,sample_answers\r\n" +
      "A,Fine?,,\r\n" +
      "A,,some variant,\r\n" + // line 3: no question
      `A,${"x".repeat(1001)},,\r\n`; // line 4: too long
    const r = parseQuestionWorkbook(buf(csv), "x.csv");
    expect(r.questions).toHaveLength(1);
    expect(r.errors).toEqual([
      { row: 3, message: "Missing question text." },
      { row: 4, message: "Question is longer than 1000 characters." },
    ]);
  });

  it("groups rows by set (case-insensitive); rows without a set use the fallback", () => {
    const csv = "question_set,question\r\nSales,Q1\r\nsales,Q2\r\nHR,Q3\r\n,Q4\r\n";
    const sets = groupBySet(parseQuestionWorkbook(buf(csv), "x.csv").questions, "from-file");
    expect(sets.map((s) => [s.name, s.questions.length])).toEqual([["Sales", 2], ["HR", 1], ["from-file", 1]]);
  });

  it("a file without a question_set column still imports (older exports)", () => {
    const r = parseQuestionWorkbook(buf("question,sample_answers\r\nQ1,A1\r\n"), "Sales intake.csv");
    expect(groupBySet(r.questions, nameFromFilename("Sales intake.csv"))[0].name).toBe("Sales intake");
  });

  it("builds safe filenames", () => {
    expect(csvFilename("Sales/intake: v2")).toBe("Sales_intake_ v2.csv");
    expect(csvFilename("   ")).toBe("question-set.csv");
  });
});
