'use client';

import { ListChecks, Mic, FileText } from 'lucide-react';
import { useMeetingSession } from '@/lib/meeting/session';
import type { MeetingKind } from '@/lib/meeting/phase';

const CHOICES: { kind: MeetingKind; title: string; blurb: string; Icon: typeof Mic }[] = [
  {
    kind: 'standard',
    title: 'Standard meeting',
    blurb: 'Record with live transcription. Summarise afterwards with any summary type.',
    Icon: FileText,
  },
  {
    kind: 'copilot',
    title: 'Meeting with co-pilot',
    blurb:
      'Pick a question set. Questions stay beside the live transcript, each tagged as it is covered, and a question summary is ready when you finish.',
    Icon: ListChecks,
  },
];

/** First screen: choose the meeting type. Both open the same meeting shell. */
export function NewMeeting() {
  const { setKind } = useMeetingSession();
  return (
    <div className="flex h-screen flex-col bg-gray-50">
      <div className="flex flex-1 items-center justify-center p-8">
        <div className="w-full max-w-3xl">
          <div className="flex items-center gap-3">
            <div className="flex h-10 w-10 items-center justify-center rounded-full bg-blue-600">
              <Mic className="h-5 w-5 text-white" />
            </div>
            <h1 className="text-2xl font-semibold text-gray-900">New meeting</h1>
          </div>
          <p className="mt-2 text-sm text-gray-500">Choose how you want to run this meeting.</p>
          <div className="mt-6 grid gap-4 sm:grid-cols-2">
            {CHOICES.map(({ kind, title, blurb, Icon }) => (
              <button
                key={kind}
                type="button"
                onClick={() => setKind(kind)}
                className="rounded-xl border border-gray-200 bg-white p-6 text-left transition hover:border-blue-400 hover:shadow-sm"
              >
                <div className="flex items-center gap-2 text-lg font-medium text-gray-900">
                  <Icon className="h-5 w-5 text-gray-600" />
                  {title}
                </div>
                <p className="mt-2 text-sm text-gray-600">{blurb}</p>
              </button>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
