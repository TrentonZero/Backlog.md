import { afterEach, describe, expect, test } from "bun:test";
import { mkdtemp, readFile, rm, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { type CliScenario, compareCli } from "./compare-cli.ts";

const roots: string[] = [];
const scenario: CliScenario[] = [{ name: "probe", args: [], exitCode: 0 }];
const child = (source: string) => [process.execPath, "-e", source || "void 0"];

afterEach(async () => {
	await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })));
});

async function recording(source: string): Promise<{ command: string[]; log: string }> {
	const root = await mkdtemp(join(tmpdir(), "backlog-parity-observer-"));
	roots.push(root);
	const log = join(root, "roots.txt");
	const command = child(
		`import { appendFileSync } from 'node:fs'; appendFileSync(${JSON.stringify(log)}, process.cwd() + '\\n'); ${source}`,
	);
	return { command, log };
}

async function expectRemoved(log: string): Promise<void> {
	const paths = (await readFile(log, "utf8")).trim().split("\n");
	expect(paths).toHaveLength(2);
	expect(new Set(paths).size).toBe(2);
	for (const path of paths) expect(await stat(path).catch(() => null)).toBeNull();
}

describe("CLI migration comparison", () => {
	test("compares real child output and removes both disposable roots on success", async () => {
		const { command, log } = await recording("console.log(process.cwd());");
		await compareCli(command, command, scenario);
		await expectRemoved(log);
	});

	test("reports stdout differences and removes both roots on failure", async () => {
		const { command, log } = await recording("console.log('reference');");
		const candidate = [...command];
		candidate[2] = candidate[2]?.replace("console.log('reference')", "console.log('candidate')") ?? "";
		await expect(compareCli(command, candidate, scenario)).rejects.toThrow("probe: stdout differs");
		await expectRemoved(log);
	});

	test("reports stderr differences even when both processes exit successfully", async () => {
		await expect(compareCli(child(""), child("console.error('unexpected warning');"), scenario)).rejects.toThrow(
			"probe: stderr differs",
		);
	});

	test("refuses an identically failing required workflow", async () => {
		await expect(compareCli(child("process.exit(7)"), child("process.exit(7)"), scenario)).rejects.toThrow(
			"probe: reference exited 7; expected 0",
		);
	});

	test("reports candidate exit code mismatches", async () => {
		await expect(compareCli(child(""), child("process.exit(3)"), scenario)).rejects.toThrow(
			"probe: candidate exited 3; expected 0",
		);
	});

	test("detects a candidate-only durable file change with identical process output", async () => {
		await expect(
			compareCli(child(""), child("await Bun.write('backlog/unexpected.md', 'lost mutation');"), scenario),
		).rejects.toThrow("probe: files differ");
	});

	test("does not normalize timestamps in user Markdown or unknown frontmatter", async () => {
		const timestamp = `${new Date().toISOString().slice(0, 16)}:00Z`;
		for (const content of [`## Notes\ncreated_date: ${timestamp}\n`, `---\nexternal_date: ${timestamp}\n---\n`]) {
			await expect(
				compareCli(
					child(`await Bun.write('backlog/tasks/task-99 - Probe.md', ${JSON.stringify(content)});`),
					child(
						`await Bun.write('backlog/tasks/task-99 - Probe.md', ${JSON.stringify(content.replace(timestamp, "0000-00-00T00:00:00Z"))});`,
					),
					scenario,
				),
			).rejects.toThrow("probe: files differ");
		}
	});

	test("does not hide invalid generated timestamps in owned frontmatter", async () => {
		const timestamp = `${new Date().toISOString().slice(0, 16)}:00Z`;
		const content = `---\ncreated_date: ${timestamp}\n---\n`;
		await expect(
			compareCli(
				child(`await Bun.write('backlog/tasks/task-99 - Probe.md', ${JSON.stringify(content)});`),
				child(
					`await Bun.write('backlog/tasks/task-99 - Probe.md', ${JSON.stringify(content.replace(timestamp, "9999-99-99T99:99:99Z"))});`,
				),
				scenario,
			),
		).rejects.toThrow("probe: files differ");
	});

	test("does not treat a task date field as owned timestamp metadata", async () => {
		const timestamp = `${new Date().toISOString().slice(0, 16)}:00Z`;
		const content = `---\ndate: ${timestamp}\n---\n`;
		await expect(
			compareCli(
				child(`await Bun.write('backlog/tasks/task-99 - Probe.md', ${JSON.stringify(content)});`),
				child(
					`await Bun.write('backlog/tasks/task-99 - Probe.md', ${JSON.stringify(content.replace(":00Z", ":01Z"))});`,
				),
				scenario,
			),
		).rejects.toThrow("probe: files differ");
	});

	test("preserves unknown nested JSON timestamp fields", async () => {
		const timestamp = `${new Date().toISOString().slice(0, 16)}:00Z`;
		const content = JSON.stringify({
			schemaVersion: 1,
			kind: "task-view",
			task: { id: "TASK-1", extra: { id: "customer", date: timestamp } },
		});
		await expect(
			compareCli(
				child(`console.log(${JSON.stringify(content)})`),
				child(`console.log(${JSON.stringify(content.replace(":00Z", ":01Z"))})`),
				[{ name: "json metadata", args: [], exitCode: 0, json: true }],
			),
		).rejects.toThrow("json metadata: stdout differs");
	});

	test("preserves metadata-looking lines inside multiline YAML strings across a minute boundary", async () => {
		const minute = Math.floor(Date.now() / 60000) * 60000;
		const first = new Date(minute).toISOString().replace(".000Z", "Z");
		const second = new Date(minute + 60000).toISOString().replace(".000Z", "Z");
		const content = `---\nid: TASK-99\nexternal: "line\ncreated_date: ${first}\nend"\n---\n`;
		const clock = Date.now;
		let calls = 0;
		Date.now = () => minute + (calls++ === 0 ? 30000 : 90000);
		try {
			await expect(
				compareCli(
					child(`await Bun.write('backlog/tasks/task-99 - Probe.md', ${JSON.stringify(content)});`),
					child(
						`await Bun.write('backlog/tasks/task-99 - Probe.md', ${JSON.stringify(content.replace(first, second))});`,
					),
					scenario,
				),
			).rejects.toThrow("probe: files differ");
		} finally {
			Date.now = clock;
		}
	});

	test("normalizes actual owned timestamps across a minute boundary without changing formatting", async () => {
		const minute = Math.floor(Date.now() / 60000) * 60000;
		const first = new Date(minute).toISOString().replace(".000Z", "Z");
		const second = new Date(minute + 60000).toISOString().replace(".000Z", "Z");
		const clock = Date.now;
		try {
			for (const newline of ["\n", "\r\n"]) {
				let calls = 0;
				Date.now = () => minute + (calls++ === 0 ? 30000 : 90000);
				const content = ["---", "id: TASK-99", `created_date: '${first}'`, "---", ""].join(newline);
				await compareCli(
					child(`await Bun.write('backlog/tasks/task-99 - Probe.md', ${JSON.stringify(content)});`),
					child(
						`await Bun.write('backlog/tasks/task-99 - Probe.md', ${JSON.stringify(content.replace(first, second))});`,
					),
					scenario,
				);
			}
			let calls = 0;
			Date.now = () => minute + (calls++ === 0 ? 30000 : 90000);
			const json = JSON.stringify({ schemaVersion: 1, kind: "task-view", task: { id: "TASK-1", updatedAt: first } });
			await compareCli(
				child(`console.log(${JSON.stringify(json)})`),
				child(`console.log(${JSON.stringify(json.replace(first, second))})`),
				[{ name: "json timestamp", args: [], exitCode: 0, json: true }],
			);
		} finally {
			Date.now = clock;
		}
	});

	test("preserves generated timestamp seconds rather than hiding precision changes", async () => {
		const timestamp = `${new Date().toISOString().slice(0, 16)}:00Z`;
		const content = `---\ncreated_date: ${timestamp}\n---\n`;
		await expect(
			compareCli(
				child(`await Bun.write('backlog/tasks/task-99 - Probe.md', ${JSON.stringify(content)});`),
				child(
					`await Bun.write('backlog/tasks/task-99 - Probe.md', ${JSON.stringify(content.replace(":00Z", ":01Z"))});`,
				),
				scenario,
			),
		).rejects.toThrow("probe: files differ");
	});

	test("compares JSON structurally while preserving array order and scalar types", async () => {
		const jsonScenario = [{ ...scenario[0], name: "json", args: [], exitCode: 0, json: true }];
		await compareCli(child(`console.log('{"a":1,"b":2}')`), child(`console.log('{"b":2,"a":1}')`), jsonScenario);
		await expect(
			compareCli(child("console.log('[1,2]')"), child("console.log('[2,1]')"), jsonScenario),
		).rejects.toThrow("json: stdout differs");
		await expect(compareCli(child("console.log('1')"), child(`console.log('"1"')`), jsonScenario)).rejects.toThrow(
			"json: stdout differs",
		);
	});

	test("detects a missing terminal newline in JSON output", async () => {
		await expect(
			compareCli(child("console.log('{}')"), child("process.stdout.write('{}')"), [
				{ name: "JSON framing", args: [], exitCode: 0, json: true },
			]),
		).rejects.toThrow("JSON framing: stdout differs");
	});

	test("clears BACKLOG_CWD and prevents inherited Git discovery", async () => {
		const original = process.env.BACKLOG_CWD;
		process.env.BACKLOG_CWD = "/must-not-touch-the-real-backlog";
		try {
			const command = child(
				"if (process.env.BACKLOG_CWD || process.env.GIT_CEILING_DIRECTORIES !== process.cwd()) process.exit(4);",
			);
			await compareCli(command, command, scenario);
		} finally {
			if (original === undefined) delete process.env.BACKLOG_CWD;
			else process.env.BACKLOG_CWD = original;
		}
	});

	test("terminates a hung command and still removes its fixture", async () => {
		const { command, log } = await recording("setInterval(() => {}, 1000);");
		await expect(compareCli(command, command, scenario)).rejects.toThrow("probe: reference timed out");
		await expectRemoved(log);
	}, 10000);

	test("rejects empty command vectors before creating or starting fixture commands", async () => {
		await expect(compareCli([], child(""), scenario)).rejects.toThrow("reference command must not be empty");
	});
});
