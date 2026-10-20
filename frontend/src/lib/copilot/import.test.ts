import { describe, expect, it } from "vitest";
import * as XLSX from "xlsx";
import {
  ImportError,
  MAX_FILE_BYTES,
  MAX_ROWS,
  parseQuestionWorkbook,
  splitMultiValue,
} from "./import";

function xlsxBuffer(aoa: unknown[][]): ArrayBuffer {
  const ws = XLSX.utils.aoa_to_sheet(aoa);
  const wb = XLSX.utils.book_new();
  XLSX.utils.book_append_sheet(wb, ws, "Questions");
  return XLSX.write(wb, { type: "array", bookType: "xlsx" }) as ArrayBuffer;
}

function csvBuffer(text: string): ArrayBuffer {
  return new TextEncoder().encode(text).buffer as ArrayBuffer;
}

describe("parseQuestionWorkbook", () => {
  it("keeps a comma-containing question as one item, not two", () => {
    const result = parseQuestionWorkbook(
      csvBuffer("question,sample_answers\n\"Approved by Q3, pending CFO\",\"Yes, by Q3; CFO sign-off\"\n"),
      "questions.csv",
    );
    expect(result.errors).toHaveLength(0);
    expect(result.questions).toHaveLength(1);
    expect(result.questions[0].main_question).toBe("Approved by Q3, pending CFO");
    expect(result.questions[0].sample_answers).toEqual(["Yes, by Q3; CFO sign-off"]);
  });

  it("splits multi-value cells on | only", () => {
    expect(splitMultiValue("a | b|c")).toEqual(["a", "b", "c"]);
    expect(splitMultiValue("a, b")).toEqual(["a, b"]);
    expect(splitMultiValue("")).toEqual([]);
  });

  it("accepts case-insensitive header aliases", () => {
    const result = parseQuestionWorkbook(
      xlsxBuffer([
        ["Main Question", "Variants", "Rubric"],
        ["What is the salary?", "Compensation?|Pay?", "50k base"],
      ]),
      "q.xlsx",
    );
    expect(result.questions[0].main_question).toBe("What is the salary?");
    expect(result.questions[0].sample_questions).toEqual(["Compensation?", "Pay?"]);
    expect(result.questions[0].sample_answers).toEqual(["50k base"]);
  });

  it("rejects a file without a question header", () => {
    expect(() => parseQuestionWorkbook(xlsxBuffer([["foo"], ["bar"]]), "bad.xlsx")).toThrow(
      ImportError,
    );
  });

  it("rejects a file larger than 2 MB", () => {
    const big = new ArrayBuffer(MAX_FILE_BYTES + 1);
    expect(() => parseQuestionWorkbook(big, "big.csv")).toThrow(/too large/i);
  });

  it("rejects more than 500 data rows", () => {
    const aoa: unknown[][] = [["question"]];
    for (let i = 0; i < MAX_ROWS + 1; i++) aoa.push([`q${i}`]);
    expect(() => parseQuestionWorkbook(xlsxBuffer(aoa), "many.xlsx")).toThrow(/too many rows/i);
  });

  it("does not evaluate formula cells", () => {
    const ws = XLSX.utils.aoa_to_sheet([["question"], [""]]);
    // A formula with no cached value must never be computed by the importer.
    ws["A2"] = { t: "n", f: "1+1" } as XLSX.CellObject;
    const wb = XLSX.utils.book_new();
    XLSX.utils.book_append_sheet(wb, ws, "Questions");
    const buf = XLSX.write(wb, { type: "array", bookType: "xlsx" }) as ArrayBuffer;
    const result = parseQuestionWorkbook(buf, "formula.xlsx");
    expect(result.questions.map((q) => q.main_question)).not.toContain("2");
  });

  it("reports per-row errors for blank question text on a populated row", () => {
    const result = parseQuestionWorkbook(
      xlsxBuffer([
        ["question", "sample_answers"],
        ["", "rubric only"],
        ["real question", ""],
      ]),
      "mixed.xlsx",
    );
    expect(result.errors).toHaveLength(1);
    expect(result.errors[0].row).toBe(2); // header is row 1
    expect(result.questions).toHaveLength(1);
  });
});
