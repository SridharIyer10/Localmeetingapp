// UI end-to-end check of the unified meeting flow against a mocked Tauri backend
// (headless Chrome). The mock returns the same JSON shapes as the Rust commands and
// has a working event bus, so transcripts stream in and the recorder start/stop
// are simulated. It proves the front end; the Rust side is covered by cargo tests.
//   npm i --no-save puppeteer-core && (cd frontend && pnpm dev) &
//   node scripts/e2e-ui.mjs           # every line must say "true"
// Needs Google Chrome at the path below; screenshots land in the working directory.
import puppeteer from 'puppeteer-core';

const CHROME = '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const browser = await puppeteer.launch({ executablePath: CHROME, headless: 'new', args: ['--no-sandbox'] });
const page = await browser.newPage();
await page.setViewport({ width: 1500, height: 950 });
const problems = [];
page.on('pageerror', (e) => problems.push(`[pageerror] ${e.message}`));
page.on('dialog', (d) => d.accept());
let failures = 0;
const out = (label, ok) => { if (!ok) failures++; console.log(label.padEnd(66), ok); };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

await page.evaluateOnNewDocument(() => {
  const db = {
    sets: [], items: [], questions: [], savedFiles: [], calls: [],
    rec: false, paused: false, customTypes: [], analysisStage: null,
    summaryProcess: null,
  };
  window.__db = db;
  let cbId = 1;
  const callbacks = new Map();
  const listeners = {};
  window.__emit = (event, payload) => (listeners[event] ?? []).forEach((h) => callbacks.get(h)?.({ event, id: 0, payload }));
  const parse = (s) => JSON.parse(s || '[]');
  const uniq = (n) => n + '-' + Math.random().toString(36).slice(2, 7);
  const norm = (s) => s.trim().toLowerCase();

  const addItem = (setId, question, sq, sa) => {
    if (!question.trim()) throw 'question cannot be empty';
    if (!db.sets.find((g) => g.id === setId)) throw 'question bank not found';
    const id = uniq('item');
    db.items.push({ id, bank_id: setId, question: question.trim(), sample_questions_json: JSON.stringify(sq), sample_answers_json: JSON.stringify(sa) });
    return id;
  };
  const tokens = (t) => norm(t).replace(/[^a-z0-9 ]/g, ' ').split(/\s+/).filter((w) => w.length >= 4 && !['what', 'does', 'your', 'this', 'with'].includes(w));
  const STATUS = (q, texts) => {
    const hay = texts.join(' ').toLowerCase();
    const hit = tokens(q.main_question).filter((t) => hay.includes(t));
    if (hit.length === 0) return 'not_asked';
    return hit.length * 2 >= tokens(q.main_question).length ? 'answered' : 'asked_not_answered';
  };
  const handlers = {
    // --- question sets
    question_bank_group_list: () => db.sets,
    question_bank_group_create: ({ name }) => {
      if (!name.trim()) throw 'question bank name cannot be empty';
      if (db.sets.some((g) => norm(g.name) === norm(name))) throw `a question bank named '${name.trim()}' already exists`;
      const id = uniq('set'); db.sets.push({ id, name: name.trim(), created_at: 'x', updated_at: 'x' }); return id;
    },
    question_bank_group_rename: ({ id, name }) => { db.sets.find((g) => g.id === id).name = name.trim(); },
    question_bank_group_delete: ({ id }) => { db.sets = db.sets.filter((g) => g.id !== id); db.items = db.items.filter((i) => i.bank_id !== id); },
    question_bank_list: () => db.items,
    question_bank_add: ({ question, sampleQuestions, sampleAnswers, bankId }) => addItem(bankId, question, sampleQuestions, sampleAnswers),
    question_bank_delete: ({ id }) => { db.items = db.items.filter((i) => i.id !== id); },
    question_set_import: ({ name, questions }) => {
      if (!questions.length) throw 'no questions to import';
      let set = db.sets.find((g) => norm(g.name) === norm(name)); const created = !set;
      if (!set) { set = { id: uniq('set'), name: name.trim(), created_at: 'x', updated_at: 'x' }; db.sets.push(set); }
      db.items = db.items.filter((i) => i.bank_id !== set.id);
      questions.forEach((q) => addItem(set.id, q.main_question, q.sample_questions, q.sample_answers));
      return { id: set.id, created, count: questions.length };
    },
    save_text_file: ({ suggestedName, content }) => { db.savedFiles.push({ name: suggestedName, content }); return '/tmp/' + suggestedName; },
    // --- summary types
    api_list_templates: () => [
      { id: 'standard_meeting', name: 'Standard Meeting Notes', description: 'General meetings', is_custom: false, prompt: null },
      { id: 'daily_standup', name: 'Daily Standup', description: 'Standups', is_custom: false, prompt: null },
      ...db.customTypes,
    ],
    api_save_custom_template: ({ id, name, prompt }) => {
      const all = ['standard meeting notes', 'daily standup', ...db.customTypes.filter((t) => t.id !== id).map((t) => norm(t.name))];
      if (all.includes(norm(name))) throw `A summary type named '${name.trim()}' already exists`;
      if (id) { const t = db.customTypes.find((x) => x.id === id); t.name = name.trim(); t.prompt = prompt.trim(); return id; }
      const nid = uniq('custom_t'); db.customTypes.push({ id: nid, name: name.trim(), description: 'Custom summary type', is_custom: true, prompt: prompt.trim() }); return nid;
    },
    api_delete_custom_template: ({ id }) => { db.customTypes = db.customTypes.filter((t) => t.id !== id); },
    api_get_template_details: () => ({ sections: ['Summary'], section_details: [{ title: 'Summary', instruction: 'Provide a brief executive summary' }] }),
    // --- meeting questions & manual tags
    copilot_begin_session: ({ draftId, setId }) => {
      const items = db.items.filter((i) => i.bank_id === setId);
      if (!items.length) throw 'this question set has no questions';
      db.questions = db.questions.filter((q) => q.meeting_id !== draftId);
      items.forEach((b, n) => db.questions.push({
        id: uniq('q'),
        meeting_id: draftId,
        sort_order: n,
        main_question: b.question,
        sample_questions: parse(b.sample_questions_json),
        sample_answers: parse(b.sample_answers_json),
        status: 'not_asked',
        rating: null,
        extracted_answer: null,
        missing_details: null,
        confidence: null,
        evidence_segment_ids: [],
        user_notes: null,
        manually_overridden: false,
        user_tag: null,
        tag_note: null,
      }));
      db.calls.push(['begin', draftId]);
      return db.questions.filter((q) => q.meeting_id === draftId);
    },
    copilot_list_questions: ({ meetingId }) => db.questions.filter((q) => q.meeting_id === meetingId),
    copilot_list_segments: () => db.segments ?? [],
    copilot_set_question_tag: ({ questionId, tag }) => {
      if (db.analysisStage && db.analysisStage !== 'failed') throw 'tags are locked once the summary has been generated';
      const q = db.questions.find((x) => x.id === questionId);
      if (q) q.user_tag = tag ?? null;
      db.calls.push(['tag', questionId, tag ?? null]);
    },
    copilot_set_question_notes: ({ questionId, notes }) => { db.questions.find((q) => q.id === questionId).user_notes = notes; },
    copilot_override_verdict: ({ questionId, status, rating }) => { const q = db.questions.find((x) => x.id === questionId); Object.assign(q, { status, rating, manually_overridden: true }); db.calls.push(['override', questionId, status]); },
    copilot_finalize_recording: (a) => {
      db.calls.push(['finalize', a]);
      db.questions.forEach((q) => { if (q.meeting_id === a.copilotMeetingId) q.meeting_id = a.savedMeetingId; });
      db.segments = (db.texts ?? []).map((t, i) => ({ id: 'seg-' + i, meeting_id: a.savedMeetingId, seq: i, start_ms: i * 4000, end_ms: i * 4000 + 3000, channel: 1, speaker_label: null, text: t }));
      return db.segments.length;
    },
    copilot_run_analysis: async ({ meetingId }) => {
      db.calls.push(['analysis', meetingId]);
      db.analysisStage = 'evaluating_questions';
      await new Promise((r) => setTimeout(r, 600));
      let modelCalls = 0;
      db.questions.filter((q) => q.meeting_id === meetingId).forEach((q) => {
        if (q.manually_overridden) return;
        const st = STATUS(q, db.texts ?? []);
        if (q.user_tag === 'not_asked' && st === 'not_asked') {
          q.status = 'not_asked';
          q.extracted_answer = 'Not asked in this call.';
          q.evidence_segment_ids = [];
          q.tag_note = null;
          return;
        }
        modelCalls++;
        q.status = st;
        q.extracted_answer = st === 'answered' ? 'Answered. The range discussed for this role was 50,000 with a signing bonus.' : null;
        q.evidence_segment_ids = st === 'not_asked' ? [] : ['t1', 't2'];
        if (!q.user_tag) {
          q.tag_note = 'Not explicitly tagged.';
        } else if (q.user_tag === 'not_asked' && st !== 'not_asked') {
          q.tag_note = 'Tagged Not asked, but the transcript shows this question was discussed.';
        } else {
          q.tag_note = null;
        }
      });
      db.analysisStage = 'complete';
      return { questions: db.questions.length, engine: 'ollama/qwen2.5:7b', note: null, model_calls: modelCalls };
    },
    copilot_analysis_status: () => ({ stage: db.analysisStage, engine: db.analysisStage === 'complete' ? 'ollama/qwen2.5:7b' : null }),
    // --- standard summary pipeline (Part B)
    api_get_model_config: () => ({ provider: 'ollama', model: 'qwen2.5:7b', whisperModel: 'small' }),
    get_ollama_models: () => [{ name: 'qwen2.5:7b' }],
    api_process_transcript: ({ meetingId, templateId }) => {
      db.calls.push(['summary-run', meetingId, templateId]);
      const qs = db.questions.filter((q) => q.meeting_id === meetingId);
      const qaBlock = qs.length
        ? [
            '## Questions & Answers',
            '',
            '1. **What is the base salary?** — *Asked & Answered*',
            '   The base salary discussed for this role was 50k with a signing bonus.',
            '',
            '2. **What is the notice period?** — *Not Asked*',
            '   Not discussed in this call.',
            '',
            '3. **How big is the team?** — *Not Asked*',
            '   Not discussed in this call.',
            '',
          ].join('\n')
        : '';
      const markdown =
        qaBlock +
        [
          '## Overview',
          '',
          'Candidate intake call covering compensation and team scope.',
          '',
          '## Key Points',
          '',
          '- Base salary is 50k with a signing bonus.',
          '',
          '## Decisions',
          '',
          '- Proceed with grade 4 compensation band.',
          '',
          '## Action Items',
          '',
          '- Send role specification by Friday.',
          '',
          '## Next Steps',
          '',
          '- Schedule technical interview.',
        ].join('\n');
      db.summaryProcess = {
        status: 'completed',
        meeting_id: meetingId,
        meetingName: 'Saved call',
        data: { markdown },
      };
      return { message: 'started', process_id: 'proc-1' };
    },
    api_get_summary: () => db.summaryProcess ?? { status: 'idle', meetingName: 'Saved call', data: null },
    // --- recorder + saving
    start_recording_with_devices_and_meeting: () => { db.rec = true; db.paused = false; db.calls.push(['recorder-start']); window.__emit('recording-started', {}); return null; },
    stop_recording: () => { db.rec = false; db.calls.push(['recorder-stop']); return null; },
    pause_recording: () => { db.paused = true; return null; },
    resume_recording: () => { db.paused = false; return null; },
    get_recording_state: () => ({ is_recording: db.rec, is_paused: db.paused, is_active: db.rec && !db.paused, recording_duration: db.rec ? 5 : null, active_duration: db.rec ? 5 : null }),
    is_recording: () => db.rec,
    get_transcription_status: () => ({ is_processing: false, chunks_in_queue: 0, last_activity_ms: 9000 }),
    api_save_transcript: ({ transcripts }) => { db.calls.push(['save', transcripts.length]); db.meetings = [{ id: 'meeting-1', title: 'Saved call', created_at: '2026-01-01T00:00:00Z', updated_at: '2026-01-01T00:00:00Z', folder_path: null }]; return { status: 'success', meeting_id: 'meeting-1' }; },
    api_get_meetings: () => db.meetings ?? [],
    api_get_meeting: () => ({ id: 'meeting-1', title: 'Saved call', created_at: '2026-01-01T00:00:00Z', updated_at: '2026-01-01T00:00:00Z', transcripts: [] }),
    api_get_meeting_metadata: ({ meetingId }) => ({ id: meetingId, title: 'Saved call', created_at: '2026-01-01T00:00:00Z', updated_at: '2026-01-01T00:00:00Z', folder_path: null }),
    api_get_meeting_transcripts: () => ({ transcripts: [{ id: 't-1', text: 'The base salary is 50k with a signing bonus.', timestamp: '10:00:01', audio_start_time: 0, audio_end_time: 3, duration: 3 }], total_count: 1, has_more: false }),
    get_recording_meeting_name: () => 'Call',
    get_meeting_folder_path: () => null,
    get_audio_devices: () => [{ name: 'Mic', device_type: 'Input' }],
    get_onboarding_status: () => ({ version: '1', completed: true, current_step: 4, model_status: { parakeet: 'downloaded', summary: 'downloaded' }, last_updated: 'x' }),
    check_first_launch: () => false,
    builtin_ai_is_model_ready: () => true,
    parakeet_has_available_models: () => true,
    api_get_transcript_config: () => ({ provider: 'parakeet', model: 'parakeet' }),
  };
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main', windowLabel: 'main' } },
    transformCallback: (cb) => { const id = cbId++; callbacks.set(id, cb); return id; },
    unregisterCallback: (id) => callbacks.delete(id),
    convertFileSrc: (p) => p,
    invoke: async (cmd, args) => {
      if (cmd in handlers) { try { return await handlers[cmd](args || {}); } catch (e) { throw typeof e === 'string' ? e : String(e); } }
      if (cmd === 'plugin:event|listen') { (listeners[args.event] ??= []).push(args.handler); return args.handler; }
      if (cmd === 'plugin:event|unlisten') return null;
      if (/devices|_list$|^list_|available_models|get_.*_models?$/.test(cmd)) return [];
      return null;
    },
  };
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
  localStorage.setItem('onboarding_completed', 'true');
});

const text = () => page.evaluate(() => document.body.innerText);
async function click(label) {
  const h = await page.evaluateHandle((label) => { const all = [...document.querySelectorAll('button, a, [role=tab]')]; return all.find((x) => x.innerText.trim().startsWith(label)) ?? all.find((x) => x.innerText.includes(label)) ?? null; }, label);
  const el = h.asElement();
  if (!el) throw new Error('no button: ' + label);
  await el.click();
}
const nav = (url) => page.evaluate((u) => window.next.router.push(u), url);
const db = (fn) => page.evaluate(fn);
async function waitText(t, timeout = 15000) { await page.waitForFunction((t) => document.body.innerText.includes(t), { timeout }, t); }

// ============ SETTINGS: summary types ============
await page.goto('http://localhost:3118/settings', { waitUntil: 'networkidle2', timeout: 60000 });
await page.waitForFunction(() => document.body.innerText.includes('Summarization'), { timeout: 20000 });
await page.addStyleTag({ content: '[data-sonner-toaster],[data-sonner-toast]{pointer-events:none !important}' });
await click('Summarization');
await waitText('Summary types');
let t = await text();
out('ST1. lists built-in summary types', t.includes('Standard Meeting Notes') && t.includes('Daily Standup') && t.includes('Built-in'));
await click('New summary type');
await page.type('input[aria-label="Summary type name"]', 'Risk review');
await page.type('textarea[aria-label="Summary type prompt"]', 'List every risk raised, who owns it and the mitigation agreed.');
await click('Add summary type');
await sleep(300);
t = await text();
out('ST2. custom type added and labelled Custom', t.includes('Risk review') && t.includes('Custom'));

// ============ SETTINGS: question sets ============
await page.type('input[aria-label="New question set name"]', 'Sales intake');
await click('Create question set');
await sleep(300);
await click('Add question');
await page.type('input[placeholder="Question"]', 'What is the base salary?');
await page.type('input[placeholder^="Sample questions"]', 'How much is the compensation?');
await page.type('input[placeholder^="Sample answers"]', 'About 50k');
await click('Add to Sales intake');
await sleep(300);
await click('Add question');
await page.type('input[placeholder="Question"]', 'What is the notice period?');
await click('Add to Sales intake');
await sleep(300);
await click('Add question');
await page.type('input[placeholder="Question"]', 'How big is the team?');
await click('Add to Sales intake');
await sleep(300);
t = await text();
out('QS1. set created with 3 questions (header + count)', t.includes('Sales intake') && t.includes('3 questions'));

// ============ CO-PILOT MEETING: manual tagging + two-part summary ============
await db(() => { window.__db.calls.length = 0; window.__db.rec = false; window.__db.summaryProcess = null; });
await nav('/');
await waitText('New meeting');
await click('Meeting with co-pilot');
await sleep(500);
const salesId = await db(() => window.__db.sets.find((s) => s.name === 'Sales intake').id);
await page.select('select[aria-label="Question set"]', salesId);
await sleep(500);

const tagBtns = await page.$$eval('aside[aria-label="Questions"] [data-question] button[data-tag-option]', (els) => els.map((e) => e.innerText.trim()));
out('C1. each question has ASKED | NOT ASKED | ANSWERED buttons', tagBtns.slice(0, 3).join('|') === 'ASKED|NOT ASKED|ANSWERED');

const clickAria = async (label) => { await page.waitForSelector(`button[aria-label="${label}"]:not([disabled])`, { timeout: 10000 }); await page.click(`button[aria-label="${label}"]`); };
const say = (seq, textLine, start) => page.evaluate(({ seq, textLine, start }) => window.__emit('transcript-update', { text: textLine, timestamp: '10:00:0' + seq, source: 'mic', sequence_id: seq, chunk_start_time: start, is_partial: false, confidence: 0.9, audio_start_time: start, audio_end_time: start + 3, duration: 3 }), { seq, textLine, start });

await clickAria('Start recording');
await sleep(800);
await say(1, 'Welcome, thanks for joining. Let us talk about compensation.', 0);
await say(2, 'What is the base salary for this role?', 4);
await say(3, 'The base salary is 50k with a signing bonus.', 8);
await sleep(600);

// No live status or model calls during the call: all questions remain untagged until user taps.
const autoTagged = await page.$$eval('aside[aria-label="Questions"] [data-question]', (els) => els.map((e) => e.dataset.tag));
out('C2. no live AI during call: questions stay untagged until user taps', autoTagged.every((x) => x === 'untagged'));

// User manually tags Q1 as ANSWERED and Q2 as NOT ASKED, leaves Q3 untagged.
await page.click('aside [data-question]:nth-child(1) button[data-tag-option="answered"]');
await page.click('aside [data-question]:nth-child(2) button[data-tag-option="not_asked"]');
await sleep(300);
out('C3. manual tags persisted with the session', await db(() => {
  const qs = window.__db.questions;
  return qs[0]?.user_tag === 'answered' && qs[1]?.user_tag === 'not_asked' && qs[2]?.user_tag === null;
}));

// Stop recording -> opens meeting-details; unified summary runs in background and renders one section.
await db(() => { window.__db.texts = ['Welcome, thanks for joining.', 'What is the base salary for this role?', 'The base salary is 50k with a signing bonus.']; window.__db.analysisStage = null; });
await clickAria('Stop recording');
await page.waitForFunction(() => location.pathname.startsWith('/meeting-details'), { timeout: 40000 });
await waitText('Questions & Answers', 15000);
await waitText('Candidate intake call covering compensation', 15000);
t = await text();
out('P1. background summary ran automatically after call ended', await db(() => window.__db.calls.some((c) => c[0] === 'summary-run')));
out('P2. single section: no separate Part A / Part B split', !t.includes('Part A —') && !t.includes('Part B —'));
out('P3. Questions & Answers precede Overview in the single document', t.indexOf('Questions & Answers') < t.indexOf('Overview'));
out('P4. Answered question has interpretive prose answer', t.includes('The base salary discussed for this role was 50k with a signing bonus.'));
out('P5. Not asked question says "Not discussed in this call."', t.includes('Not discussed in this call.'));

// Download (single unified document) and Save (transcript as .txt)
await clickAria('Download summary');
await sleep(300);
const dl = await db(() => window.__db.savedFiles.at(-1));
out('D1. Download saves unified summary as one local file (title + date)', dl?.name === 'Saved-call-2026-01-01.md' && dl.content.includes('Questions & Answers') && dl.content.includes('Overview'));

await clickAria('Save transcript');
await sleep(300);
const tr = await db(() => window.__db.savedFiles.at(-1));
out('D2. Save saves full transcript as .txt locally (title + date)', tr?.name === 'Saved-call-2026-01-01.txt' && tr.content.includes('The base salary is 50k'));

const bad = problems.filter((l) => !/ResizeObserver/.test(l));
console.log('page errors:', bad.length); bad.slice(0, 5).forEach((l) => console.log('  ' + l));
console.log(failures === 0 && bad.length === 0 ? '\nALL CHECKS PASSED' : `\n${failures} CHECK(S) FAILED`);
await browser.close();
process.exit(failures === 0 && bad.length === 0 ? 0 : 1);
