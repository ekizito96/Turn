# Software factory benchmark

This benchmark measures whether Turn materially reduces the application infrastructure required to move a software issue into a tested pull request while retaining crash recovery and an audit trail.

It is not a model-quality benchmark. The current proof uses deterministic mock inference and decision providers so results reflect workflow semantics rather than paid-model latency or accuracy.

## Factory stages

1. Evaluate requirement completeness and security risk through `decide`.
2. Generate an implementation plan through `infer`.
3. Create a branch.
4. Apply a patch.
5. Run tests.
6. Evaluate merge readiness and human-review need through `decide`.
7. Open one pull request.

The Turn workflow is [`examples/software_factory.tn`](../../examples/software_factory.tn). Its executable proof is [`impl/tests/software_factory_test.rs`](../../impl/tests/software_factory_test.rs).

## Current verified behavior

The integration test injects a crash after the `open_pull_request` outcome has been committed to the effect journal but before the VM resumes. On restart:

- The pending effect ID finds its completed journal record.
- `open_pull_request` is not invoked again.
- The workflow returns `pull_request_opened`.
- Seven effects remain queryable in execution order.
- The effect-aware PR handler receives the stable ID it can forward to GitHub or another external deduplication boundary.

Run the proof:

```bash
cargo test --locked --test software_factory_test -- --nocapture
```

## Comparison protocol

Implement the same stages in each candidate stack without paid APIs:

- Turn with `FileStore`, mock LLM/decision providers, and fake repository tools.
- Plain Python or TypeScript with hand-built checkpoint and journal logic.
- Temporal local development server with activities and workflow history.
- Restate local runtime with durable handlers.

Each implementation must inject the same failure after the pull-request service accepts the request and before workflow progress is persisted.

## Metrics

| Metric | Definition |
|---|---|
| Application SLOC | Nonblank, noncomment workflow and integration code excluding generated files |
| Infrastructure components | Processes/services required to execute and recover the workflow |
| Setup commands | Commands required from clean checkout to passing recovery proof |
| Duplicate external invocations | Calls to `open_pull_request` across crash and restart |
| Recovery latency | Time from restart to completed workflow result |
| Checkpoint bytes | Durable bytes written before the pull-request effect |
| Journal bytes | Durable effect-history bytes after completion |
| Audit completeness | Whether all seven effects expose identity, tool, outcome, and ordering |
| External idempotency support | Whether the stable effect ID reaches the integration handler |

## Fairness rules

- Use the same mock delays and payload sizes.
- Separate framework/runtime code from application code.
- Report warm and cold startup separately.
- Do not count model output quality when all providers are mocked.
- Do not claim exactly-once external behavior unless the external mock deduplicates the supplied effect ID.
- Publish raw commands, source, and machine specifications with results.

## Remaining work

The Turn proof is implemented. Conventional, Temporal, and Restate baselines are not yet implemented, so no comparative superiority claim is made here yet.
