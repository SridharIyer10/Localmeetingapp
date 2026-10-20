import { useCallback, RefObject } from 'react';
import { MeetingSummary, Transcript } from '@/types';
import { BlockNoteSummaryViewRef } from '@/components/AISummary/BlockNoteSummaryView';
import { toast } from 'sonner';
import { invoke as invokeTauri } from '@tauri-apps/api/core';
import { hasVisibleSummaryContent } from '@/lib/summary-content';
import { saveTextFile } from '@/lib/copilot/api';
import { defaultExportFilename } from '@/lib/copilot/summaryMarkdown';

interface UseCopyOperationsProps {
  meeting: any;
  transcripts: Transcript[];
  meetingTitle: string;
  aiSummary: MeetingSummary | null;
  blockNoteSummaryRef: RefObject<BlockNoteSummaryViewRef>;
}

const formatTranscriptTime = (seconds: number | undefined, fallbackTimestamp: string): string => {
  if (seconds === undefined) {
    return fallbackTimestamp;
  }
  const totalSecs = Math.floor(seconds);
  const mins = Math.floor(totalSecs / 60);
  const secs = totalSecs % 60;
  return `[${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}]`;
};

export function useCopyOperations({
  meeting,
  transcripts,
  meetingTitle,
  aiSummary,
  blockNoteSummaryRef,
}: UseCopyOperationsProps) {

  // Helper function to fetch ALL transcripts (not just paginated data)
  const fetchAllTranscripts = useCallback(async (meetingId: string): Promise<Transcript[]> => {
    try {
      const firstPage = await invokeTauri('api_get_meeting_transcripts', {
        meetingId,
        limit: 1,
        offset: 0,
      }) as { transcripts: Transcript[]; total_count: number; has_more: boolean };

      const totalCount = firstPage.total_count;
      if (totalCount === 0) {
        return [];
      }

      const allData = await invokeTauri('api_get_meeting_transcripts', {
        meetingId,
        limit: totalCount,
        offset: 0,
      }) as { transcripts: Transcript[]; total_count: number; has_more: boolean };

      return allData.transcripts;
    } catch (error) {
      console.error('❌ Error fetching all transcripts:', error);
      toast.error('Failed to fetch transcripts');
      return [];
    }
  }, []);

  const buildFullTranscriptText = useCallback(async (): Promise<string | null> => {
    const allTranscripts = await fetchAllTranscripts(meeting.id);
    const list = allTranscripts.length > 0 ? allTranscripts : transcripts;
    if (!list.length) {
      toast.error('No transcripts available');
      return null;
    }
    const header = `# Transcript of the Meeting: ${meeting.id} - ${meetingTitle ?? meeting.title}\n\n`;
    const date = `## Date: ${new Date(meeting.created_at).toLocaleDateString()}\n\n`;
    const fullTranscript = list
      .map(t => `${formatTranscriptTime(t.audio_start_time, t.timestamp)} ${t.text}  `)
      .join('\n');
    return header + date + fullTranscript;
  }, [fetchAllTranscripts, meeting, meetingTitle, transcripts]);

  // Copy transcript to clipboard
  const handleCopyTranscript = useCallback(async () => {
    const content = await buildFullTranscriptText();
    if (!content) return;
    await navigator.clipboard.writeText(content);
    toast.success("Transcript copied to clipboard");
  }, [buildFullTranscriptText]);

  // Save full transcript as .txt locally via native save dialog
  const handleSaveTranscript = useCallback(async () => {
    const content = await buildFullTranscriptText();
    if (!content) return;
    try {
      const filename = defaultExportFilename(meetingTitle || meeting.title, meeting.created_at, 'txt');
      const savedPath = await saveTextFile(filename, content);
      if (savedPath) {
        toast.success('Transcript saved', { description: savedPath });
      }
    } catch (error) {
      console.error('❌ Failed to save transcript:', error);
      toast.error('Failed to save transcript');
    }
  }, [buildFullTranscriptText, meeting, meetingTitle]);

  const extractPartBMarkdown = useCallback(async (): Promise<string> => {
    let summaryMarkdown = '';
    if (blockNoteSummaryRef.current?.getMarkdown) {
      summaryMarkdown = await blockNoteSummaryRef.current.getMarkdown();
    }
    if (!summaryMarkdown && aiSummary && typeof aiSummary.markdown === 'string') {
      summaryMarkdown = aiSummary.markdown;
    }
    if (!summaryMarkdown && aiSummary) {
      const sections = Object.entries(aiSummary)
        .filter(([key]) => {
          return key !== 'markdown' && key !== 'summary_json' && key !== '_section_order' && key !== 'MeetingName';
        })
        .map(([, section]) => {
          if (section && typeof section === 'object' && 'title' in section && 'blocks' in section) {
            const sectionTitle = `## ${section.title}\n\n`;
            const sectionContent = section.blocks
              .map((block: any) => `- ${block.content}`)
              .join('\n');
            return sectionTitle + sectionContent;
          }
          return '';
        })
        .filter(s => s.trim())
        .join('\n\n');
      summaryMarkdown = sections;
    }
    return summaryMarkdown;
  }, [aiSummary, blockNoteSummaryRef]);

  // Copy summary to clipboard
  const handleCopySummary = useCallback(async () => {
    if (!hasVisibleSummaryContent(aiSummary)) {
      toast.error('No summary content available to copy');
      return;
    }
    try {
      const summaryMarkdown = await extractPartBMarkdown();
      if (!summaryMarkdown.trim()) {
        toast.error('No summary content available to copy');
        return;
      }

      const fullMarkdown = `# Meeting Summary: ${meetingTitle || meeting.title}\n\n**Meeting ID:** ${meeting.id}\n**Date:** ${new Date(meeting.created_at).toLocaleDateString('en-US', {
        year: 'numeric',
        month: 'long',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit'
      })}\n\n---\n\n${summaryMarkdown}`;

      await navigator.clipboard.writeText(fullMarkdown);
      toast.success("Summary copied to clipboard");
    } catch (error) {
      console.error('❌ Failed to copy summary:', error);
      toast.error("Failed to copy summary");
    }
  }, [aiSummary, extractPartBMarkdown, meeting, meetingTitle]);

  // Download the unified summary as one file locally
  const handleDownloadSummary = useCallback(async () => {
    if (!hasVisibleSummaryContent(aiSummary)) {
      toast.error('No summary content available to download');
      return;
    }
    try {
      const summaryMarkdown = await extractPartBMarkdown();
      const dateStr = meeting.created_at ? new Date(meeting.created_at).toISOString().slice(0, 10) : '';
      const content = `# ${meetingTitle || meeting.title}\n${dateStr ? `*Date: ${dateStr}*\n` : ''}\n${summaryMarkdown.trim()}\n`;
      const filename = defaultExportFilename(meetingTitle || meeting.title, meeting.created_at, 'md');
      const savedPath = await saveTextFile(filename, content);
      if (savedPath) {
        toast.success('Summary downloaded', { description: savedPath });
      }
    } catch (error) {
      console.error('❌ Failed to download summary:', error);
      toast.error('Failed to download summary');
    }
  }, [aiSummary, extractPartBMarkdown, meeting, meetingTitle]);

  return {
    handleCopyTranscript,
    handleSaveTranscript,
    handleCopySummary,
    handleDownloadSummary,
  };
}
