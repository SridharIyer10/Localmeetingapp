'use client';

import { useEffect } from 'react';
import { useMeetingEngine } from '@/hooks/useMeetingEngine';
import { useMeetingSession } from '@/lib/meeting/session';
import { MeetingShell } from '@/components/meeting/MeetingShell';
import { NewMeeting } from '@/components/meeting/NewMeeting';

export default function Home() {
  const engine = useMeetingEngine();
  const { kind, phase, reset } = useMeetingSession();

  useEffect(() => {
    // Arriving with nothing in progress always starts from the chooser.
    if (phase === 'idle' || phase === 'done') reset();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Recording started from the tray/sidebar has no chosen kind: it is a standard meeting.
  if (kind === null && phase === 'idle') return <NewMeeting />;
  return <MeetingShell engine={engine} />;
}
