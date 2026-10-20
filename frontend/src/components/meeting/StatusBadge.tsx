import { STATUS_LABEL, STATUS_STYLE } from "@/lib/copilot/status";
import type { QuestionStatus } from "@/lib/copilot/types";

export function StatusBadge({ status }: { status: QuestionStatus }) {
  return (
    <span
      data-status={status}
      className={`inline-block whitespace-nowrap rounded border px-2 py-0.5 text-xs font-medium ${STATUS_STYLE[status]}`}
    >
      {STATUS_LABEL[status]}
    </span>
  );
}
