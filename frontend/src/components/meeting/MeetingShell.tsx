'use client';

import { RecordingControls } from '@/components/RecordingControls';
import { StatusOverlays } from '@/app/_components/StatusOverlays';
import { SettingsModals } from '@/app/_components/SettingsModal';
import { TranscriptPanel } from '@/app/_components/TranscriptPanel';
import { TranscriptRecovery } from '@/components/TranscriptRecovery';
import { RecordingStatus } from '@/contexts/RecordingStateContext';
import { useMeetingEngine } from '@/hooks/useMeetingEngine';
import { PHASE_LABEL } from '@/lib/meeting/phase';
import { useMeetingSession } from '@/lib/meeting/session';
import { QuestionPanel } from './QuestionPanel';
import { SetPicker } from './SetPicker';

type Engine = ReturnType<typeof useMeetingEngine>;

const PHASE_STYLE = {
  idle: 'bg-gray-100 text-gray-700',
  recording: 'bg-red-100 text-red-700',
  paused: 'bg-amber-100 text-amber-800',
  processing: 'bg-blue-100 text-blue-700',
  done: 'bg-green-100 text-green-700',
} as const;

/**
 * The one meeting screen. Standard and co-pilot meetings render this same shell:
 * header, transcript, controls, overlays. A co-pilot meeting adds the question
 * set picker and the question panel on the left; nothing else differs.
 */
export function MeetingShell({ engine }: { engine: Engine }) {
  const { kind, phase, setId, setKind } = useMeetingSession();
  const copilot = kind === 'copilot';
  const needsSet = copilot && !setId;

  return (
    <div className="flex h-screen flex-col bg-gray-50">
      <SettingsModals modals={engine.modals} messages={engine.messages} onClose={engine.hideModal} />
      <TranscriptRecovery
        isOpen={engine.recovery.isOpen}
        onClose={engine.recovery.onClose}
        recoverableMeetings={engine.recovery.recoverableMeetings}
        onRecover={engine.recovery.onRecover}
        onDelete={engine.recovery.onDelete}
        onLoadPreview={engine.recovery.onLoadPreview}
      />

      <header className="flex shrink-0 flex-wrap items-center gap-3 border-b border-gray-200 bg-white px-4 py-2">
        {phase === 'idle' && (
          <button type="button" className="text-sm text-gray-500" onClick={() => setKind(null)}>
            ← Back
          </button>
        )}
        <h1 className="text-base font-semibold text-gray-900">
          {copilot ? 'Meeting with co-pilot' : 'Standard meeting'}
        </h1>
        <span
          data-phase={phase}
          className={`rounded px-2 py-0.5 text-xs font-medium ${PHASE_STYLE[phase]}`}
          aria-live="polite"
        >
          {PHASE_LABEL[phase]}
        </span>
        {copilot && (
          <div className="ml-auto">
            <SetPicker />
          </div>
        )}
      </header>

      <div className="flex min-h-0 flex-1 overflow-hidden">
        {copilot && <QuestionPanel />}
        <TranscriptPanel
          isProcessingStop={engine.isProcessingStop}
          isStopping={engine.isStopping}
          showModal={engine.showModal}
        />

        {(engine.hasMicrophone || engine.isRecording) &&
          engine.status !== RecordingStatus.PROCESSING_TRANSCRIPTS &&
          engine.status !== RecordingStatus.SAVING && (
            <div className="fixed bottom-12 left-0 right-0 z-10">
              <div
                className="flex justify-center pl-8 transition-[margin] duration-300"
                style={{ marginLeft: engine.sidebarCollapsed ? '4rem' : '16rem' }}
              >
                <div className="flex w-2/3 max-w-[750px] flex-col items-center gap-2">
                  {needsSet && phase === 'idle' && (
                    <p className="rounded bg-amber-50 px-3 py-1 text-xs text-amber-800 shadow">
                      Choose a question set to start the co-pilot meeting.
                    </p>
                  )}
                  <div className="flex items-center rounded-full bg-white shadow-lg">
                    <RecordingControls
                      isRecording={engine.recordingState.isRecording}
                      onRecordingStop={engine.stopMeeting}
                      onRecordingStart={engine.startMeeting}
                      onTranscriptReceived={() => {}}
                      onStopInitiated={() => engine.setIsStopping(true)}
                      barHeights={engine.barHeights}
                      onTranscriptionError={(message) => engine.showModal('errorAlert', message)}
                      isRecordingDisabled={engine.isRecordingDisabled || needsSet}
                      isParentProcessing={engine.isProcessingStop}
                      selectedDevices={engine.selectedDevices}
                      meetingName={engine.meetingTitle}
                    />
                  </div>
                </div>
              </div>
            </div>
          )}

        <StatusOverlays
          isProcessing={engine.status === RecordingStatus.PROCESSING_TRANSCRIPTS && !engine.recordingState.isRecording}
          isSaving={engine.status === RecordingStatus.SAVING}
          sidebarCollapsed={engine.sidebarCollapsed}
        />
      </div>
    </div>
  );
}
