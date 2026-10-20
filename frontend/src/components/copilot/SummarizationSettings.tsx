'use client';

import QuestionSetManager from './QuestionSetManager';
import SummaryTypesSettings from './SummaryTypesSettings';

/** Settings → Summarization: summary types and co-pilot question sets. */
export default function SummarizationSettings() {
  return (
    <div className="flex flex-col gap-6">
      <section className="rounded-lg border border-gray-200 bg-gray-50 p-6 shadow-sm">
        <h3 className="mb-1 text-lg font-semibold text-gray-900">Summary types</h3>
        <p className="mb-4 text-sm text-gray-600">
          Every kind of summary the app can write. Add your own with a name and a prompt; it appears in every summary-type
          menu, for standard and co-pilot meetings alike.
        </p>
        <SummaryTypesSettings />
      </section>

      <section className="rounded-lg border border-gray-200 bg-gray-50 p-6 shadow-sm">
        <h3 className="mb-1 text-lg font-semibold text-gray-900">Co-pilot question sets</h3>
        <p className="mb-4 text-sm text-gray-600">
          A question set is the list you pick when you start a meeting with the co-pilot. Download the CSV template, fill it
          in and import it, or export a set to edit it elsewhere; importing a file whose set name already exists replaces
          that set.
        </p>
        <QuestionSetManager />
      </section>
    </div>
  );
}
