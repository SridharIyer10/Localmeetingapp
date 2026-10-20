// The five session states shared by every meeting type. Derived, never stored:
// the recording state (synced from the backend) is the single source of truth.

export type MeetingPhase = "idle" | "recording" | "paused" | "processing" | "done";
export type MeetingKind = "standard" | "copilot";

/** Mirrors `RecordingStatus` in RecordingStateContext (string values). */
export type RecordingStatusValue =
  | "idle"
  | "starting"
  | "recording"
  | "stopping"
  | "processing"
  | "saving"
  | "completed"
  | "error";

export function derivePhase(input: {
  status: RecordingStatusValue;
  isRecording: boolean;
  isPaused: boolean;
}): MeetingPhase {
  const { status, isRecording, isPaused } = input;
  if (status === "stopping" || status === "processing" || status === "saving") return "processing";
  if (isRecording) return isPaused ? "paused" : "recording";
  if (status === "starting" || status === "recording") return "recording";
  if (status === "completed") return "done";
  return "idle"; // idle, and error (the session is startable again)
}

export const PHASE_LABEL: Record<MeetingPhase, string> = {
  idle: "Ready",
  recording: "Recording",
  paused: "Paused",
  processing: "Processing",
  done: "Saved",
};
