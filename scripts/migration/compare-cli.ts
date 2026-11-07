import { mkdir, mkdtemp, readdir, readFile, readlink, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { isDeepStrictEqual } from "node:util";
import matter from "gray-matter";

export interface CliScenario {
	name: string;
	args: readonly string[];
	exitCode: number;
	json?: boolean;
}

const scenarios: readonly CliScenario[] = [
	{ name: "task list", args: ["task", "list", "--json"], exitCode: 0, json: true },
	{ name: "task detail", args: ["task", "view", "1", "--json"], exitCode: 0, json: true },
	{ name: "plain detail", args: ["task", "view", "1", "--plain"], exitCode: 0 },
	{ name: "filtered list", args: ["task", "list", "--status", "Done", "--json"], exitCode: 0, json: true },
	{ name: "missing task", args: ["task", "view", "999", "--json"], exitCode: 1 },
	{ name: "output conflict", args: ["task", "list", "--json", "--plain"], exitCode: 1 },
	{ name: "config read", args: ["config", "get", "projectName"], exitCode: 0 },
	{ name: "documents", args: ["doc", "list", "--plain"], exitCode: 0 },
	{ name: "decisions", args: ["decision", "list", "--json"], exitCode: 0, json: true },
	{ name: "search", args: ["search", "Migration", "--json"], exitCode: 0, json: true },
	{ name: "edit task", args: ["task", "edit", "1", "--check-ac", "1", "--add-label", "verified"], exitCode: 0 },
	{ name: "edited detail", args: ["task", "view", "1", "--json"], exitCode: 0, json: true },
	{ name: "create task", args: ["task", "create", "Migration child", "--ac", "Round trip"], exitCode: 0 },
	{ name: "created list", args: ["task", "list", "--json"], exitCode: 0, json: true },
	{ name: "archive task", args: ["task", "archive", "3"], exitCode: 0 },
	{ name: "archived detail", args: ["task", "view", "3", "--json"], exitCode: 0, json: true },
	{ name: "refuse archived edit", args: ["task", "edit", "3", "--title", "Must not change"], exitCode: 1 },
	{ name: "restore task", args: ["task", "restore", "3"], exitCode: 0 },
	{ name: "archive before delete", args: ["task", "archive", "3"], exitCode: 0 },
	{ name: "delete archived task", args: ["task", "delete", "3"], exitCode: 0 },
	{ name: "create draft", args: ["draft", "create", "Migration draft"], exitCode: 0 },
	{ name: "draft list", args: ["draft", "list", "--json"], exitCode: 0, json: true },
	{ name: "promote draft", args: ["draft", "promote", "DRAFT-1"], exitCode: 0 },
	{ name: "decision edit", args: ["decision", "edit", "decision-1", "--context", "Migration context"], exitCode: 0 },
	{ name: "decision detail", args: ["decision", "view", "decision-1", "--json"], exitCode: 0, json: true },
	{ name: "document archive", args: ["doc", "archive", "doc-1"], exitCode: 0 },
	{ name: "document restore", args: ["doc", "restore", "doc-1"], exitCode: 0 },
	{ name: "milestone add", args: ["milestone", "add", "Migration release"], exitCode: 0 },
	{ name: "milestone list", args: ["milestone", "list", "--plain"], exitCode: 0 },
];

async function seed(root: string): Promise<void> {
	for (const directory of ["tasks", "drafts", "docs", "decisions", "milestones", "completed"]) {
		await mkdir(join(root, "backlog", directory), { recursive: true });
	}
	await writeFile(
		join(root, "backlog/config.yml"),
		'project_name: "Migration fixture"\nstatuses: ["To Do", "In Progress", "Done"]\nlabels: []\ndefault_status: "To Do"\ndate_format: yyyy-mm-dd hh:mm\nfilesystem_only: true\nremote_operations: false\ncheck_active_branches: false\nauto_commit: false\ntask_prefix: task\n',
	);
	await writeFile(
		join(root, "backlog/tasks/task-1 - Migration-task.md"),
		'---\nid: TASK-1\ntitle: Migration task\nstatus: To Do\nassignee: ["@maintainer"]\ncreated_date: 2020-01-02\nlabels: [migration]\ndependencies: [TASK-2]\nexternal_data: {keep: "unchanged"}\n---\n\n## Description\n\n<!-- SECTION:DESCRIPTION:BEGIN -->\nMigration task body with `code` and @mentions.\n<!-- SECTION:DESCRIPTION:END -->\n\n## Acceptance Criteria\n\n<!-- AC:BEGIN -->\n- [ ] #1 Preserve files\n<!-- AC:END -->\n',
	);
	await writeFile(
		join(root, "backlog/tasks/task-2 - Migration-dependency.md"),
		"---\nid: TASK-2\ntitle: Migration dependency\nstatus: Done\nassignee: []\ncreated_date: 2020-01-01\nlabels: []\ndependencies: []\n---\n",
	);
	await writeFile(
		join(root, "backlog/docs/doc-1 - Migration-guide.md"),
		"---\nid: doc-1\ntitle: Migration guide\ntype: guide\ncreated_date: 2020-01-01\n---\n\nMigration documentation.\n",
	);
	await writeFile(
		join(root, "backlog/decisions/decision-1 - Migration-decision.md"),
		"---\nid: decision-1\ntitle: Migration decision\ndate: 2020-01-01\nstatus: proposed\n---\n\n## Context\n\n<!-- SECTION:CONTEXT:BEGIN -->\nMigration evidence.\n<!-- SECTION:CONTEXT:END -->\n",
	);
}

function normalizeTimestamp(value: string, started: number): string {
	if (!/^\d{4}-\d{2}-\d{2}(?:[T ]\d{2}:\d{2}(?::\d{2})?Z?)?$/.test(value)) return value;
	const dayOnly = value.length === 10;
	const precision = dayOnly ? 86400000 : 60000;
	const instant = Date.parse(!dayOnly && !value.endsWith("Z") ? `${value}Z` : value);
	if (!Number.isFinite(instant) || instant < Math.floor(started / precision) * precision || instant > Date.now())
		return value;
	// Retain precision, separators and timezone spelling; only clock-owned digits vary.
	return value.slice(0, 16).replace(/\d/g, "0") + value.slice(16);
}

function normalizePaths(text: string, root: string): string {
	return text.replaceAll(root, "<project>").replaceAll(root.replaceAll("\\", "/"), "<project>");
}

function normalizeFile(text: string, path: string, started: number): string {
	const kind = path.match(/^backlog\/(?:archive\/)?(tasks|drafts|docs|decisions)\/.+\.md$/)?.[1];
	if (!kind) return text;
	const owned = kind === "decisions" ? ["date", "updated_date"] : ["created_date", "updated_date"];
	return text.replace(/^(\uFEFF?---\r?\n)([\s\S]*?)(\r?\n---(?=\r?\n|$))/, (_, open, yaml, close) => {
		// Parser spans distinguish root fields from matching text inside user scalars.
		// Keep the original bytes everywhere except the owned timestamp's clock digits.
		let raw = `\n${yaml}`;
		const starts: number[] = [];
		let offset = 0;
		for (const line of raw.split(/(?<=\n)/)) {
			starts.push(offset);
			offset += line.length;
		}
		type Node = { start: number; end: number; result: unknown; kind: string; children: Node[] };
		const stack: Node[] = [];
		const mappings: Node[] = [];
		try {
			const parsed = matter(`---${raw}\n---`, {
				language: "yaml",
				listener(
					event: string,
					state: { position: number; line: number; lineStart: number; result: unknown; kind: string },
				) {
					const position = (starts[state.line] ?? raw.length) + state.position - state.lineStart;
					if (event === "open") {
						stack.push({ start: position, end: position, result: undefined, kind: "", children: [] });
					} else {
						const node = stack.pop();
						if (!node) return;
						node.end = position;
						node.result = state.result;
						node.kind = state.kind;
						stack.at(-1)?.children.push(node);
						if (node.kind === "mapping") mappings.push(node);
					}
				},
			});
			const entries = mappings.find((node) => node.result === parsed.data)?.children ?? [];
			for (let index = entries.length - 2; index >= 0; index--) {
				const key = entries[index];
				const value = entries[index + 1];
				if (!key || !value || !owned.includes(String(key.result)) || value.kind !== "scalar") continue;
				if (!raw.slice(key.end, value.start).trimStart().startsWith(":")) continue;
				const source = raw.slice(value.start, value.end);
				const normalized = source.replace(
					/^(\s*)(["']?)([^\r\n]*?)\2(\s*)$/,
					(_, space, quote, timestamp, suffix) =>
						`${space}${quote}${normalizeTimestamp(timestamp, started)}${quote}${suffix}`,
				);
				raw = raw.slice(0, value.start) + normalized + raw.slice(value.end);
			}
			return `${open}${raw.slice(1)}${close}`;
		} catch {
			// Unsupported or invalid YAML is compared exactly, never silently rewritten.
			return `${open}${yaml}${close}`;
		}
	});
}

function normalizeJson(value: unknown, started: number): unknown {
	if (!value || typeof value !== "object" || Array.isArray(value)) return value;
	const envelope = value as Record<string, unknown>;
	if (envelope.schemaVersion !== 1) return value;
	const record = (input: unknown, fields: readonly string[]): unknown => {
		if (!input || typeof input !== "object" || Array.isArray(input)) return input;
		const result = { ...input } as Record<string, unknown>;
		for (const field of fields) {
			if (typeof result[field] === "string") result[field] = normalizeTimestamp(result[field], started);
		}
		return result;
	};
	const list = (input: unknown, fields: readonly string[]): unknown =>
		Array.isArray(input) ? input.map((item) => record(item, fields)) : input;
	const dates = ["createdAt", "updatedAt"];
	switch (envelope.kind) {
		case "task-list":
			return { ...envelope, tasks: list(envelope.tasks, dates) };
		case "draft-list":
			return { ...envelope, drafts: list(envelope.drafts, dates) };
		case "task-view":
			return { ...envelope, task: record(envelope.task, dates) };
		case "decision-list":
			return { ...envelope, decisions: list(envelope.decisions, ["date"]) };
		case "decision-view":
			return { ...envelope, decision: record(envelope.decision, ["date", "updatedAt"]) };
		case "search":
			return {
				...envelope,
				results: Array.isArray(envelope.results)
					? envelope.results.map((item) => {
							if (!item || typeof item !== "object") return item;
							return { ...item, data: record(item.data, item.type === "decision" ? ["date"] : dates) };
						})
					: envelope.results,
			};
		default:
			return value;
	}
}

async function snapshot(root: string, started: number, directory = ""): Promise<Record<string, string>> {
	const files: Record<string, string> = {};
	const entries = await readdir(join(root, directory), { withFileTypes: true });
	for (const entry of entries.sort((a, b) => a.name.localeCompare(b.name))) {
		const path = directory ? `${directory}/${entry.name}` : entry.name;
		if (entry.isDirectory()) {
			files[`${path}/`] = "directory";
			Object.assign(files, await snapshot(root, started, path));
		} else if (entry.isSymbolicLink()) {
			files[path] = `link:${normalizePaths(await readlink(join(root, path)), root)}`;
		} else {
			const content = await readFile(join(root, path));
			files[path] = /\.(md|ya?ml)$/.test(path)
				? normalizeFile(normalizePaths(content.toString("utf8"), root), path, started)
				: content.toString("base64");
		}
	}
	return files;
}

async function run(command: readonly string[], args: readonly string[], root: string) {
	const env = { ...process.env, GIT_CEILING_DIRECTORIES: root, NO_COLOR: "1", FORCE_COLOR: "0", TERM: "dumb" };
	delete (env as Record<string, string | undefined>).BACKLOG_CWD;
	for (const key of ["GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_COMMON_DIR"]) {
		delete (env as Record<string, string | undefined>)[key];
	}
	const child = Bun.spawn([...command, ...args], { cwd: root, env, stdin: "ignore", stdout: "pipe", stderr: "pipe" });
	let timedOut = false;
	const timer = setTimeout(() => {
		timedOut = true;
		child.kill("SIGKILL");
	}, 5000);
	try {
		const [exitCode, stdout, stderr] = await Promise.all([
			child.exited,
			new Response(child.stdout).text(),
			new Response(child.stderr).text(),
		]);
		return { exitCode, stdout: normalizePaths(stdout, root), stderr: normalizePaths(stderr, root), timedOut };
	} finally {
		clearTimeout(timer);
	}
}

/** Compare external executables without importing either product implementation. */
export async function compareCli(
	reference: readonly string[],
	candidate: readonly string[],
	cases: readonly CliScenario[] = scenarios,
): Promise<void> {
	for (const [name, command] of [
		["reference", reference],
		["candidate", candidate],
	] as const) {
		if (!command[0]) throw new Error(`${name} command must not be empty`);
	}
	const parent = await mkdtemp(join(tmpdir(), "backlog-cli-parity-"));
	const left = join(parent, "reference");
	const right = join(parent, "candidate");
	const started = Date.now();
	try {
		await Promise.all([seed(left), seed(right)]);
		for (const scenario of cases) {
			const results = await Promise.allSettled([
				run(reference, scenario.args, left),
				run(candidate, scenario.args, right),
			]);
			const [ref, next] = results.map((result, index) => {
				const side = index === 0 ? "reference" : "candidate";
				if (result.status === "rejected")
					throw new Error(`${scenario.name}: ${side} failed to start`, { cause: result.reason });
				if (result.value.timedOut) throw new Error(`${scenario.name}: ${side} timed out`);
				if (result.value.exitCode !== scenario.exitCode) {
					throw new Error(
						`${scenario.name}: ${side} exited ${result.value.exitCode}; expected ${scenario.exitCode}\n${result.value.stderr}`,
					);
				}
				return result.value;
			});
			if (!ref || !next) throw new Error("Missing process results");
			if (scenario.json && ref.stdout.endsWith("\n") !== next.stdout.endsWith("\n")) {
				throw new Error(`${scenario.name}: stdout differs (terminal newline)`);
			}
			const stdout = (text: string) => (scenario.json ? normalizeJson(JSON.parse(text), started) : text);
			for (const field of ["stdout", "stderr"] as const) {
				const a = field === "stdout" ? stdout(ref[field]) : ref[field];
				const b = field === "stdout" ? stdout(next[field]) : next[field];
				if (!isDeepStrictEqual(a, b))
					throw new Error(
						`${scenario.name}: ${field} differs\nreference: ${JSON.stringify(a)}\ncandidate: ${JSON.stringify(b)}`,
					);
			}
			const [before, after] = await Promise.all([snapshot(left, started), snapshot(right, started)]);
			if (!isDeepStrictEqual(before, after)) {
				const paths = [...new Set([...Object.keys(before), ...Object.keys(after)])].filter(
					(path) => before[path] !== after[path],
				);
				throw new Error(`${scenario.name}: files differ: ${paths.join(", ")}`);
			}
		}
	} finally {
		await rm(parent, { recursive: true, force: true });
	}
}

if (import.meta.main) {
	try {
		const args = process.argv.slice(2);
		if (args.length !== 4 || args[0] !== "--reference" || args[2] !== "--candidate") {
			throw new Error(
				'Usage: bun scripts/migration/compare-cli.ts --reference \'["/absolute/executable", "fixed-arg"]\' --candidate \'["/absolute/executable"]\'',
			);
		}
		const commands = [args[1], args[3]].map((text) => {
			const command: unknown = JSON.parse(text ?? "");
			if (!Array.isArray(command) || !command[0] || command.some((part) => typeof part !== "string")) {
				throw new Error("Commands must be JSON string arrays with a nonempty executable");
			}
			return command.map((part: string, index: number) => (index === 0 && part.includes("/") ? resolve(part) : part));
		});
		await compareCli(commands[0] ?? [], commands[1] ?? []);
		console.log(`CLI comparison passed: ${scenarios.length} scenarios`);
	} catch (error) {
		console.error(error instanceof Error ? error.message : String(error));
		process.exitCode = 1;
	}
}
