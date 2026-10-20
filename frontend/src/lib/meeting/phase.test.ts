import { describe, expect, it } from "vitest";
import { derivePhase, type RecordingStatusValue } from "./phase";

const p = (status: RecordingStatusValue, isRecording = false, isPaused = false) =>
  derivePhase({ status, isRecording, isPaused });

describe("derivePhase", () => {
  it("maps every recording status to one of the five phases", () => {
    expect(p("idle")).toBe("idle");
    expect(p("starting")).toBe("recording");
    expect(p("recording", true)).toBe("recording");
    expect(p("recording", true, true)).toBe("paused");
    expect(p("stopping")).toBe("processing");
    expect(p("processing")).toBe("processing");
    expect(p("saving")).toBe("processing");
    expect(p("completed")).toBe("done");
    expect(p("error")).toBe("idle");
  });

  it("processing wins while the backend still reports a recording", () => {
    // Stop was pressed: the backend flag may lag the status for a moment.
    expect(p("stopping", true)).toBe("processing");
  });

  it("a live recording flag alone is enough to be recording (refresh/tray starts)", () => {
    expect(p("idle", true)).toBe("recording");
  });
});
