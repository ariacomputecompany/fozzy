# Fozzy Performance Backlog

This file tracks pedantic, production-meaningful performance and scalability improvements identified across the engine, CLI surface, profiling, and large-codebase workflows.

## Priority 0 (highest impact)

- [x] Stop scenario fuzz from invoking full `run`/`explore` pipelines per input.
   - `scenario:` fuzz currently calls full run paths that write reports/events/timelines/profile artifacts each iteration, collapsing fuzz throughput at scale.
   - Refs: `src/modes/fuzz.rs:627`, `src/modes/fuzz.rs:633`, `src/modes/fuzz.rs:655`

- [x] Make artifact generation lazy/opt-in on hot execution paths.
   - `run`, `replay`, `explore`, and fuzz crash loops currently perform heavy JSON/profile/timeline writes by default.
   - Refs: `src/runtime/engine.rs:838`, `src/runtime/engine.rs:859`, `src/modes/explore.rs:186`, `src/modes/fuzz.rs:278`

- [x] Remove duplicated run-manifest writes in the same command path.
   - Several paths write `manifest.json` multiple times for the same run artifacts.
   - Refs: `src/runtime/engine.rs:839`, `src/runtime/engine.rs:860`, `src/modes/explore.rs:187`, `src/modes/explore.rs:210`

- [x] Avoid retaining full `ScenarioRun` payloads in `test` when not needed.
   - `run_tests` stores full decisions/events/scenario data for all scenarios, scaling memory poorly on large suites.
   - Ref: `src/runtime/engine.rs:507`

- [x] Rework shrink loop to avoid clone-heavy delta-debugging.
   - Current algorithm repeatedly clones full vectors (`candidate`, `trial`), causing avoidable O(n^2+) copy pressure.
   - Ref: `src/runtime/engine.rs:1060`

## Priority 1

- [x] Remove full `ExecCtx` cloning in `assert_throws` / `assert_rejects`.
   - `exec_expect_failure` clones complete runtime state, including maps/queues/memory/events.
   - Ref: `src/runtime/engine.rs:3253`

- [x] Optimize snapshot/restore model (`fs_snapshot`, host fs snapshot/restore).
   - Virtual fs snapshots clone full state; host snapshots/restore clone and re-read/write large touched sets.
   - Refs: `src/runtime/engine.rs:2207`, `src/runtime/engine.rs:1864`, `src/runtime/engine.rs:1883`

- [x] Rework network queue/inbox structures for large message volume.
   - `net_deliver_one` scans/removes from `Vec`; `net_recv_assert` linear-scans/removes inbox items.
   - Refs: `src/runtime/engine.rs:2826`, `src/runtime/engine.rs:2839`, `src/runtime/engine.rs:2907`

- [x] Cache parsed/validated scenarios for repeated deterministic loops.
   - `doctor`, `shrink`, and replay-like paths repeatedly parse/validate identical scenarios.
   - Ref: `src/runtime/engine.rs:1391`

- [x] Precompile memory pressure-wave parsing.
    - `effective_alloc_bytes` reparses comma-separated multipliers every allocation.
    - Ref: `src/runtime/memorycap.rs:269`

- [x] Remove per-allocation heap allocation in fragmentation hashing path.
    - `Vec` is allocated each allocation to build hash input.
    - Ref: `src/runtime/memorycap.rs:287`

- [x] Make trace checksum writing less copy-heavy.
    - `TraceFile::write_json` clones full trace to compute checksum before write.
    - Ref: `src/runtime/tracefile.rs:139`

- [x] Cache commit hash/version metadata process-wide.
    - `version_info()` can shell out to `git`; trace/profile creation calls this repeatedly.
    - Refs: `src/platform/envinfo.rs:31`, `src/platform/envinfo.rs:45`, `src/runtime/tracefile.rs:72`

## Priority 2

- [x] Make profile bundle loading incremental/cached.
    - `load_profile_bundle` regenerates profile artifacts from trace and then rereads multiple JSON files.
    - Ref: `src/cmd/profile_cmd.rs:946`

- [x] Avoid repeated full profile recomputation in metric checks.
    - `metric_value` rebuilds timeline/cpu/heap/latency each invocation.
    - Refs: `src/cmd/profile_cmd.rs:1737`, `src/cmd/profile_cmd.rs:1969`

- [x] Split `profile doctor` into fast vs deep modes.
    - Current doctor includes shrink + metric recomputation + trace reads; expensive for routine checks.
    - Ref: `src/cmd/profile_cmd.rs:1828`

- [x] Reduce combinatorial cost in `map suites` coverage matching.
    - Hotspot x suite x scenario token intersections degrade on large repos.
    - Refs: `src/cmd/map_cmd.rs:314`, `src/cmd/map_cmd.rs:558`

- [x] Reduce string churn in repository scanning (`map`).
    - Per-line lowercase + repeated substring scans + full scenario lowercase/haystack generation is costly.
    - Refs: `src/cmd/map_cmd.rs:637`, `src/cmd/map_cmd.rs:914`

- [x] Avoid full run-history scans for aliases (`latest`, `last-pass`, `last-fail`).
    - Alias resolution parses all run reports each call; memory alias resolution also opens traces.
    - Refs: `src/cmd/artifacts.rs:755`, `src/cmd/memory_cmd.rs:242`

- [x] Improve file-discovery scaling in `find_matching_files`.
    - Current walk + dual rel/abs glob matching + set-dedup becomes expensive on very large trees.
    - Ref: `src/platform/fsutil.rs:12`

- [x] Avoid timeline duplication/cloning for large traces.
    - `write_timeline` builds a full duplicated event vector before writing.
    - Ref: `src/runtime/timeline.rs:18`

- [x] Improve random scheduler pop complexity.
    - `VecDeque::remove(idx)` in random mode is O(n), hurting large queue performance.
    - Ref: `src/runtime/scheduler.rs:57`

- [x] Reduce pretty-JSON overhead in hot write paths.
    - Extensive use of `to_vec_pretty` in run-time artifact emission increases CPU and I/O.
    - Refs: `src/runtime/engine.rs:838`, `src/cmd/profile_cmd.rs:2157`, `src/runtime/timeline.rs:32`

- [x] Bound large proc/http trace payload fields.
    - Event payloads can include large stdout/stderr/body strings, inflating memory and trace sizes.
    - Ref: `src/runtime/engine.rs:2678`

## Notes

- The current repository performs well on small inputs, but these items are the highest-risk scaling points for large suites, large traces, and monorepos.
