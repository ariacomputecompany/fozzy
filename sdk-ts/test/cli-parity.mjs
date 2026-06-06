import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { Fozzy } from "../dist/index.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..", "..");
const bin = process.env.FOZZY_BIN ?? path.join(repoRoot, "target", "debug", "fozzy");

function cliJson(cwd, config, args, expectedCode = 0) {
  const result = spawnSync(
    bin,
    ["--config", config, "--cwd", cwd, "--json", ...args],
    { cwd, encoding: "utf8" },
  );
  assert.equal(
    result.status,
    expectedCode,
    `cli ${args.join(" ")} exited ${result.status}\nstdout=${result.stdout}\nstderr=${result.stderr}`,
  );
  return JSON.parse(result.stdout);
}

function cliResult(cwd, config, args) {
  return spawnSync(
    bin,
    ["--config", config, "--cwd", cwd, "--json", ...args],
    { cwd, encoding: "utf8" },
  );
}

async function main() {
  assert.ok(existsSync(bin), `expected built fozzy binary at ${bin}`);

  const ws = mkdtempSync(path.join(os.tmpdir(), "fozzy-sdk-parity-"));
  try {
    copyFileSync(path.join(repoRoot, "tests", "example.fozzy.json"), path.join(ws, "example.fozzy.json"));
    copyFileSync(
      path.join(repoRoot, "tests", "memory.pass.fozzy.json"),
      path.join(ws, "memory.pass.fozzy.json"),
    );
    const config = path.join(ws, "fozzy.toml");
    writeFileSync(config, 'base_dir = ".fozzy"\n', "utf8");

    const sdk = new Fozzy({ bin, cwd: ws, config, json: true });

    assert.deepEqual(await sdk.version(), cliJson(ws, config, ["version"]));
    assert.deepEqual(await sdk.usage(), cliJson(ws, config, ["usage"]));
    assert.deepEqual(await sdk.schema(), cliJson(ws, config, ["schema"]));

    const tracePath = path.join(ws, "memory.fozzy");
    const run = await sdk.run(path.join(ws, "memory.pass.fozzy.json"), {
      det: true,
      seed: 17,
      memTrack: true,
      memArtifacts: true,
      record: tracePath,
    });
    const relativeTrace = path.relative(ws, tracePath);
    const selectors = [run.identity.runId, "latest", tracePath, relativeTrace];

    for (const selector of selectors) {
      assert.deepEqual(
        await sdk.reportShow(selector, { format: "json" }),
        cliJson(ws, config, ["report", "show", selector, "--format", "json"]),
        `report parity failed for selector ${selector}`,
      );
      assert.deepEqual(
        await sdk.memoryTop(selector),
        cliJson(ws, config, ["memory", "top", selector]),
        `memory parity failed for selector ${selector}`,
      );
      if (selector === relativeTrace) {
        const cli = cliResult(ws, config, ["artifacts", "ls", selector]);
        assert.equal(cli.status, 2, `relative trace artifacts ls should currently fail\nstdout=${cli.stdout}\nstderr=${cli.stderr}`);
        await assert.rejects(
          () => sdk.artifactsLs(selector),
          (err) =>
            err instanceof Error && err.message.includes("declared trace artifact mismatch"),
        );
      } else {
        assert.deepEqual(
          await sdk.artifactsLs(selector),
          cliJson(ws, config, ["artifacts", "ls", selector]),
          `artifacts parity failed for selector ${selector}`,
        );
      }
    }

    for (const selector of [tracePath, relativeTrace]) {
      assert.deepEqual(
        await sdk.traceVerify(selector),
        cliJson(ws, config, ["trace", "verify", selector]),
        `trace verify parity failed for selector ${selector}`,
      );
    }

    const bundlePath = path.join(ws, "bundle.zip");
    await sdk.artifactsBundle("latest", bundlePath);
    assert.ok(existsSync(bundlePath), "sdk artifactsBundle should materialize the requested bundle");

    const doctorArgs = [
      "doctor",
      "--deep",
      "--scenario",
      path.join(ws, "example.fozzy.json"),
      "--runs",
      "1",
      "--seed",
      "5",
    ];
    const doctorCli = cliJson(ws, config, doctorArgs);
    assert.deepEqual(
      await sdk.doctor({
        deep: true,
        scenario: path.join(ws, "example.fozzy.json"),
        runs: 1,
        seed: 5,
      }),
      doctorCli,
    );
  } finally {
    rmSync(ws, { recursive: true, force: true });
  }
}

await main();
