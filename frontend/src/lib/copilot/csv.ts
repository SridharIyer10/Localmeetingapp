// Question-set CSV: one schema for the template, exports and imports.
//
//   question_set,question,sample_questions,sample_answers
//
// `sample_questions` / `sample_answers` hold several values separated by `|`
// (never `,`, because questions contain commas). Importing is in import.ts.

export const CSV_COLUMNS = ["question_set", "question", "sample_questions", "sample_answers"] as const;

export interface CsvRow {
  set: string;
  question: string;
  sample_questions: string[];
  sample_answers: string[];
}

/** Stop spreadsheet apps from running a cell as a formula (CSV injection). */
function guard(text: string): string {
  return /^[=+\-@\t\r]/.test(text) ? `'${text}` : text;
}

function field(value: string): string {
  const v = guard(value);
  return /[",\r\n]/.test(v) ? `"${v.replace(/"/g, '""')}"` : v;
}

/** A literal `|` inside one value would read back as two values; keep them apart. */
const joinValues = (values: string[]) => values.map((v) => v.replace(/\|/g, "/")).join(" | ");

/** RFC 4180 CSV with CRLF line ends and a BOM so Excel opens it as UTF-8. */
export function toCsv(rows: CsvRow[]): string {
  const lines = [CSV_COLUMNS.join(",")];
  for (const r of rows) {
    lines.push(
      [r.set, r.question, joinValues(r.sample_questions), joinValues(r.sample_answers)]
        .map(field)
        .join(","),
    );
  }
  return "\uFEFF" + lines.join("\r\n") + "\r\n";
}

export const TEMPLATE_ROWS: CsvRow[] = [
  {
    set: "Sales intake",
    question: "What is your budget for this project?",
    sample_questions: ["How much can you spend?", "Is there a budget range?"],
    sample_answers: ["Around 10k", "Between 10k and 20k"],
  },
  {
    set: "Sales intake",
    question: "Who makes the final decision?",
    sample_questions: ["Who signs this off?"],
    sample_answers: ["The CFO signs off"],
  },
];

export const templateCsv = () => toCsv(TEMPLATE_ROWS);

export const csvFilename = (setName: string) =>
  `${setName.trim().replace(/[^\w\- ]+/g, "_").slice(0, 60) || "question-set"}.csv`;
