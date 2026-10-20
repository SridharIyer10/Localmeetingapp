// Client-side question-set import (CSV / XLSX) using SheetJS.
//
// Parsing happens entirely in the frontend so no file is uploaded to any local
// service. Multi-value fields accept one variant per row OR a `|` delimiter;
// they are NEVER split on `,` because question text routinely contains commas.

import * as XLSX from "xlsx";

export const MAX_FILE_BYTES = 2 * 1024 * 1024; // 2 MB
export const MAX_ROWS = 500;

export const MAX_QUESTION_CHARS = 1000;
export const MAX_ANSWER_CHARS = 2000;
export const MAX_SET_NAME_CHARS = 120;

export interface ParsedQuestion {
  /** Set the row belongs to (the `question_set` column), if the file has one. */
  question_set?: string;
  main_question: string;
  sample_questions: string[];
  sample_answers: string[];
}

export interface RowError {
  /** Line number in the file as a spreadsheet shows it (header is row 1). 0 = file-level. */
  row: number;
  message: string;
}

export interface ImportResult {
  questions: ParsedQuestion[];
  errors: RowError[];
  warnings: string[];
}

export class ImportError extends Error {}

const HEADER_ALIASES: Record<string, string[]> = {
  question_set: ["question_set", "question set", "set", "set_name", "bank", "header"],
  main_question: ["main_question", "main question", "question", "question_text"],
  sample_questions: [
    "sample_questions",
    "sample questions",
    "sample_phrasings",
    "variants",
    "alt_phrasings",
  ],
  sample_answers: [
    "sample_answers",
    "sample answers",
    "rubric",
    "expected_answer",
    "expected answers",
  ],
};

function normalizeHeader(h: string): string {
  return h.trim().toLowerCase().replace(/[-\s]+/g, "_");
}

function resolveColumns(header: string[]): Record<string, number> {
  const normalized = header.map(normalizeHeader);
  const mapping: Record<string, number> = {};
  for (const [field, aliases] of Object.entries(HEADER_ALIASES)) {
    const aliasesNormalized = aliases.map(normalizeHeader);
    const idx = normalized.findIndex((h) => aliasesNormalized.includes(h));
    if (idx >= 0) mapping[field] = idx;
  }
  return mapping;
}

/**
 * Split a multi-value cell. `|` is the only supported delimiter; text containing
 * commas stays intact.
 */
export function splitMultiValue(raw: unknown): string[] {
  if (raw === null || raw === undefined) return [];
  const text = String(raw).trim();
  if (text === "") return [];
  return text
    .split("|")
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}

/**
 * Exports prefix cells that start with = + - @ with an apostrophe so spreadsheet
 * apps cannot run them as formulas. Undo that here so a round trip is lossless.
 */
export function unguardCell(text: string): string {
  return /^'[=+\-@\t\r]/.test(text) ? text.slice(1) : text;
}

function readWorkbook(data: ArrayBuffer, filename: string): XLSX.WorkBook {
  const lower = filename.toLowerCase();
  if (lower.endsWith(".csv")) {
    const text = new TextDecoder("utf-8").decode(new Uint8Array(data));
    // cellFormula:false and raw:false ensure we only ever read cached text and
    // never evaluate a formula.
    return XLSX.read(text, { type: "string", cellFormula: false, raw: false });
  }
  return XLSX.read(data, { type: "array", cellFormula: false, raw: false });
}

/**
 * Parse an import file into questions. Throws ImportError for file-level
 * failures (too large, missing header); returns per-row errors otherwise.
 */
export function parseQuestionWorkbook(
  data: ArrayBuffer,
  filename: string,
): ImportResult {
  if (data.byteLength > MAX_FILE_BYTES) {
    throw new ImportError(
      `File is too large (${(data.byteLength / (1024 * 1024)).toFixed(1)} MB). Maximum is 2 MB.`,
    );
  }

  let workbook: XLSX.WorkBook;
  try {
    workbook = readWorkbook(data, filename);
  } catch (e) {
    throw new ImportError(`Could not read file: ${e instanceof Error ? e.message : String(e)}`);
  }

  const sheetName = workbook.SheetNames[0];
  if (!sheetName) throw new ImportError("The file contains no sheets.");
  const sheet = workbook.Sheets[sheetName];

  // header:1 gives raw rows so we can validate the header explicitly.
  const rows = XLSX.utils.sheet_to_json<unknown[]>(sheet, {
    header: 1,
    raw: false,
    defval: "",
    blankrows: false,
  });
  if (rows.length === 0) throw new ImportError("The file is empty.");

  const header = (rows[0] as unknown[]).map((c) => String(c ?? ""));
  const columns = resolveColumns(header);
  if (columns.main_question === undefined) {
    throw new ImportError(
      'Missing required "question" column. Expected a header such as "question" or "main_question".',
    );
  }

  const dataRows = rows.slice(1);
  if (dataRows.length > MAX_ROWS) {
    throw new ImportError(`Too many rows (${dataRows.length}). Maximum is ${MAX_ROWS}.`);
  }

  const questions: ParsedQuestion[] = [];
  const errors: RowError[] = [];
  const warnings: string[] = [];

  const cell = (row: unknown[], idx: number | undefined): unknown =>
    idx === undefined ? "" : row[idx];

  const text = (row: unknown[], idx: number | undefined): string =>
    unguardCell(String(cell(row, idx) ?? "").trim());

  dataRows.forEach((row, i) => {
    const rowNumber = i + 2; // header is row 1
    const arr = row as unknown[];
    const main = text(arr, columns.main_question);
    if (main === "") {
      // Skip fully-empty rows silently; error only if other cells have content.
      const hasContent = arr.some((c) => String(c ?? "").trim() !== "");
      if (hasContent) errors.push({ row: rowNumber, message: "Missing question text." });
      return;
    }
    const setName = text(arr, columns.question_set);
    const samples = splitMultiValue(text(arr, columns.sample_questions));
    const answers = splitMultiValue(text(arr, columns.sample_answers));

    const problems: string[] = [];
    if (main.length > MAX_QUESTION_CHARS) {
      problems.push(`Question is longer than ${MAX_QUESTION_CHARS} characters.`);
    }
    if (setName.length > MAX_SET_NAME_CHARS) {
      problems.push(`Question set name is longer than ${MAX_SET_NAME_CHARS} characters.`);
    }
    if (samples.some((v) => v.length > MAX_QUESTION_CHARS)) {
      problems.push(`A sample question is longer than ${MAX_QUESTION_CHARS} characters.`);
    }
    if (answers.some((v) => v.length > MAX_ANSWER_CHARS)) {
      problems.push(`A sample answer is longer than ${MAX_ANSWER_CHARS} characters.`);
    }
    if (problems.length > 0) {
      problems.forEach((message) => errors.push({ row: rowNumber, message }));
      return;
    }
    questions.push({
      ...(columns.question_set !== undefined ? { question_set: setName } : {}),
      main_question: main,
      sample_questions: samples,
      sample_answers: answers,
    });
  });

  if (questions.length === 0 && errors.length === 0) {
    warnings.push("No questions found in the file.");
  }

  return { questions, errors, warnings };
}

export interface ParsedSet {
  name: string;
  questions: ParsedQuestion[];
}

/**
 * Group parsed rows into question sets by their `question_set` value. Rows with no
 * set name go to `fallbackName` (the file name, or the set being edited).
 */
export function groupBySet(questions: ParsedQuestion[], fallbackName: string): ParsedSet[] {
  const order: string[] = [];
  const byKey = new Map<string, ParsedSet>();
  for (const q of questions) {
    const name = (q.question_set ?? "").trim() || fallbackName.trim();
    const key = name.toLowerCase();
    let set = byKey.get(key);
    if (!set) {
      set = { name, questions: [] };
      byKey.set(key, set);
      order.push(key);
    }
    set.questions.push(q);
  }
  return order.map((k) => byKey.get(k)!);
}

/** "Sales intake.csv" -> "Sales intake" */
export function nameFromFilename(filename: string): string {
  return filename.replace(/\.[^.]+$/, "").trim();
}
