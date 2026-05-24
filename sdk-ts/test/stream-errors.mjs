import assert from "node:assert/strict";
import { Fozzy } from "../dist/index.js";

const sdk = new Fozzy({ bin: "definitely-not-a-real-fozzy-bin" });

let threw = false;
try {
  for await (const _chunk of sdk.stream(["version"])) {
    // no-op
  }
} catch (err) {
  threw = true;
  assert.equal(err && err.code, "ENOENT");
}

assert.equal(threw, true, "stream() should surface spawn failures as catchable errors");
