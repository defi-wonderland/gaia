---
title: "fix: Add retry policy to proposal-executor wallet creation and setup RPC calls"
type: fix
date: 2026-04-03
---

# fix: Add retry policy to proposal-executor wallet creation and setup RPC calls

## Overview

Add Effect-level retry and timeout policies to `createSmartWallet` and `verifyExecutorSetup` RPC calls in the proposal-executor, matching the existing per-proposal retry pattern. This prevents transient RPC failures (HTTP 500) from killing the entire K8s job.

**Linear epic:** [GEO-491](https://linear.app/defi-wonderland/issue/GEO-491)

## Problem Statement

A K8s CronJob for `proposal-executor` failed because the GeoTest Pinax RPC returned HTTP 500 during smart wallet creation (`toSafeSmartAccount` → `eth_call` to `proxyCreationCode()`). The `createSmartWallet` and `verifyExecutorSetup` calls in `index.ts` have no retry policy — a single transient RPC failure aborts the entire run before any proposals are processed.

The per-proposal execution path already has retry + timeout (`executeWithRetry` in `index.ts:145-162`), but the setup phase does not.

### Error from production logs

```
Smart wallet creation failed: ContractFunctionExecutionError: HTTP request failed.
Status: 500
URL: https://geotest.rpc.pinax.network/v1/.../
Request body: {"method":"eth_call","params":[{"data":"0x53e5d935","to":"0xd9d2Ba03a7754250FDD71333F444636471CACBC4"},"latest"]}
Contract Call:
  address:   0xd9d2Ba03a7754250FDD71333F444636471CACBC4
  function:  proxyCreationCode()
```

## Proposed Solution

Apply the existing `infraRetryPolicy` (exponential backoff 1s→2s, max 2 retries) and a 30s timeout to both setup-phase RPC calls. All retry logic stays at the Effect composition level in `index.ts` — no changes to the underlying functions in `execute.ts`.

## Technical Approach

### 1. Retry `createSmartWallet` ([GEO-492](https://linear.app/defi-wonderland/issue/GEO-492))

In `index.ts`, wrap the existing call (~line 215):

```typescript
// Before
const wallet = yield* createSmartWallet({...})

// After
const wallet = yield* createSmartWallet({...}).pipe(
    Effect.timeoutFail({
        duration: Duration.seconds(30),
        onTimeout: () => new InfraError({message: "Smart wallet creation timed out after 30s", durationMs: 30_000}),
    }),
    Effect.retry({schedule: infraRetryPolicy, while: (e) => e._tag === "InfraError"}),
    Effect.withSpan("proposal-executor.create-wallet"),
)
```

### 2. Retry `verifyExecutorSetup` ([GEO-493](https://linear.app/defi-wonderland/issue/GEO-493))

Same pattern for the verification call (~line 226):

```typescript
// Before
yield* verifyExecutorSetup(wallet, config.executorSpaceId, config.spaceRegistryAddress)

// After
yield* verifyExecutorSetup(wallet, config.executorSpaceId, config.spaceRegistryAddress).pipe(
    Effect.timeoutFail({
        duration: Duration.seconds(30),
        onTimeout: () => new InfraError({message: "Executor setup verification timed out after 30s", durationMs: 30_000}),
    }),
    Effect.retry({schedule: infraRetryPolicy, while: (e) => e._tag === "InfraError"}),
    Effect.withSpan("proposal-executor.verify-setup"),
)
```

### 3. Tests ([GEO-494](https://linear.app/defi-wonderland/issue/GEO-494))

Add tests in `tests/index.test.ts` covering:
- Wallet creation recovers after transient `InfraError`
- Wallet creation fails after exhausting retries
- Setup verification recovers after transient `InfraError`
- Non-`InfraError` failures are not retried

## Design Decisions

| Decision | Rationale |
|----------|-----------|
| Effect-level retry, not viem transport retry | Consistent with existing pattern; all retry logic in one place with full observability via OTel spans |
| Reuse `infraRetryPolicy` | Same failure mode (transient RPC errors), same appropriate response (exponential backoff, 2 retries) |
| 30s timeout per attempt | Matches per-proposal timeout; prevents a hung RPC from consuming the entire 270s budget |
| No changes to `execute.ts` | Keeps functions pure; retry is a composition concern, not a business logic concern |

## Scope

**In scope:**
- Retry + timeout for `createSmartWallet` and `verifyExecutorSetup`
- Tests for retry behavior

**Out of scope:**
- RPC provider reliability investigation (separate concern)
- Fallback RPC endpoints (future enhancement if needed)
- Changes to per-proposal retry policy (already working correctly)
