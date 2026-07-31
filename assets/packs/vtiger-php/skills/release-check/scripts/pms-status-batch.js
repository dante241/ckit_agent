#!/usr/bin/env node
/**
 * pms-status-batch.js — Batch fetch PMS ticket statuses
 *
 * Reads ticket_no list from stdin (one per line, with or without '#'),
 * outputs TSV: ticket_no<TAB>status<TAB>record_id<TAB>label<TAB>categories<TAB>assignee
 *
 * Env vars required (reuses PMS MCP server config):
 *   PMS_BASE_URL, PMS_USERNAME, PMS_PASSWORD, PMS_ACCESSKEY
 *
 * Usage:
 *   cat ticket-ids.txt | node pms-status-batch.js > status.tsv
 *   node pms-status-batch.js --ids "22036,22185,17670" > status.tsv
 *
 * Concurrency: 8 parallel requests. Auto-retry once on failure.
 */

const CONCURRENCY = 8;
const MAX_RETRIES = 1;

function getEnv() {
    const { PMS_BASE_URL, PMS_USERNAME, PMS_PASSWORD, PMS_ACCESSKEY } = process.env;
    if (!PMS_BASE_URL || !PMS_USERNAME || !PMS_PASSWORD || !PMS_ACCESSKEY) {
        console.error('ERROR: Missing env vars PMS_BASE_URL/PMS_USERNAME/PMS_PASSWORD/PMS_ACCESSKEY');
        console.error('Source them from PMS MCP server config: `claude mcp get pms`');
        process.exit(2);
    }
    return {
        baseUrl: PMS_BASE_URL.replace(/\/+$/, ''),
        username: PMS_USERNAME,
        password: PMS_PASSWORD,
        accesskey: PMS_ACCESSKEY,
    };
}

async function login(creds) {
    const res = await fetch(`${creds.baseUrl}/api/CloudGoAppApi.php`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
            RequestAction: 'Login',
            Credentials: {
                username: creds.username,
                password: creds.password,
            },
        }),
    });
    if (!res.ok) throw new Error(`Login HTTP ${res.status}: ${await res.text()}`);
    const data = await res.json();
    const payload = data.data || data;
    if (!payload.token) throw new Error(`Login failed: ${payload.message || JSON.stringify(data)}`);
    return payload.token;
}

async function fetchTicketStatus(creds, token, ticketNo, retries = 0) {
    const candidates = [`#${ticketNo}`];
    if (/^\d{1,4}$/.test(ticketNo)) candidates.push(`#${ticketNo.padStart(5, '0')}`);
    try {
        for (const value of candidates) {
            const body = {
                RequestAction: 'getListTicket',
                Params: {
                    filters: [{ name: 'ticket_no', value, operator: 'e' }],
                    paging: { offset: 0, max_rows: 1 },
                },
            };
            const res = await fetch(`${creds.baseUrl}/api/CloudWorkApi.php`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'Token': token,
                    'X-Api-Version': '2',
                },
                body: JSON.stringify(body),
            });
            const data = await res.json();
            const entry = data?.data?.entry_list?.[0];
            if (!entry) continue;
            const assignee = pickAssignee(entry);
            return {
                ticket_no: ticketNo,
                status: entry.ticketstatus || 'UNKNOWN',
                record_id: entry.ticketid || entry.id || '',
                label: (entry.label || '').replace(/[\t\n\r]/g, ' '),
                categories: (entry.ticketcategories || '').replace(/[\t\n\r]/g, ' '),
                assignee: assignee.replace(/[\t\n\r]/g, ' '),
            };
        }
        return { ticket_no: ticketNo, status: 'NOT_FOUND', record_id: '', label: '', categories: '', assignee: '' };
    } catch (err) {
        if (retries < MAX_RETRIES) return fetchTicketStatus(creds, token, ticketNo, retries + 1);
        return { ticket_no: ticketNo, status: 'ERROR', record_id: '', label: String(err).slice(0, 100), categories: '', assignee: '' };
    }
}

function pickAssignee(entry) {
    const mainId = entry.main_owner_id ? String(entry.main_owner_id) : '';
    const owners = Array.isArray(entry.assigned_owners) ? entry.assigned_owners : [];
    let match = null;
    if (mainId) {
        match = owners.find((o) => String(o.id || '').endsWith(`:${mainId}`)) || null;
    }
    const raw = match ? match.name : owners[0]?.name || '';
    return raw.replace(/\s*\([^)]*\)\s*$/, '').trim();
}

async function runPool(tasks, concurrency) {
    const results = [];
    let idx = 0;
    async function worker() {
        while (idx < tasks.length) {
            const i = idx++;
            results[i] = await tasks[i]();
        }
    }
    await Promise.all(Array.from({ length: concurrency }, worker));
    return results;
}

function parseIds(argv) {
    const idsFlag = argv.find((a) => a.startsWith('--ids='));
    if (idsFlag) return idsFlag.slice(6).split(',').map((s) => s.trim().replace(/^#/, ''));
    const idx = argv.indexOf('--ids');
    if (idx >= 0 && argv[idx + 1]) return argv[idx + 1].split(',').map((s) => s.trim().replace(/^#/, ''));
    return null;
}

async function readStdin() {
    const chunks = [];
    for await (const chunk of process.stdin) chunks.push(chunk);
    return Buffer.concat(chunks)
        .toString('utf8')
        .split('\n')
        .map((l) => l.trim().replace(/^#/, ''))
        .filter((l) => /^\d{3,}$/.test(l));
}

(async () => {
    const creds = getEnv();
    let ids = parseIds(process.argv.slice(2));
    if (!ids) ids = await readStdin();
    ids = [...new Set(ids)];

    if (ids.length === 0) {
        console.error('No ticket IDs provided (stdin or --ids=)');
        process.exit(1);
    }

    console.error(`Fetching ${ids.length} tickets (concurrency=${CONCURRENCY})...`);
    const start = Date.now();

    const token = await login(creds);
    const tasks = ids.map((id) => () => fetchTicketStatus(creds, token, id));
    const results = await runPool(tasks, CONCURRENCY);

    for (const r of results) {
        process.stdout.write(
            `${r.ticket_no}\t${r.status}\t${r.record_id}\t${r.label}\t${r.categories || ''}\t${r.assignee || ''}\n`
        );
    }
    console.error(`Done in ${((Date.now() - start) / 1000).toFixed(1)}s`);
})().catch((err) => {
    console.error('FATAL:', err.message);
    process.exit(1);
});
