import type {TransactionEvent} from "@sentry/core"
import {describe, expect, it} from "vitest"

import {filterFastGraphqlDbSpans, SLOW_GRAPHQL_THRESHOLD_MS} from "./telemetry"

const FAST_MS = SLOW_GRAPHQL_THRESHOLD_MS / 2
const SLOW_MS = SLOW_GRAPHQL_THRESHOLD_MS + 500

type Span = NonNullable<TransactionEvent["spans"]>[number]

function span(partial: {
	span_id: string
	parent_span_id?: string
	durationMs?: number
	data?: Record<string, unknown>
}): Span {
	const start = 1_700_000_000
	const duration = (partial.durationMs ?? 0) / 1000
	return {
		span_id: partial.span_id,
		parent_span_id: partial.parent_span_id,
		trace_id: "t",
		op: undefined,
		description: partial.span_id,
		start_timestamp: start,
		timestamp: start + duration,
		data: partial.data,
	} as unknown as Span
}

function event(spans: Span[]): TransactionEvent {
	return {type: "transaction", spans} as TransactionEvent
}

describe("filterFastGraphqlDbSpans", () => {
	it("drops db spans whose nearest graphql ancestor was below threshold", () => {
		const result = filterFastGraphqlDbSpans(
			event([
				span({span_id: "http", data: {"http.method": "POST"}}),
				span({
					span_id: "gql",
					parent_span_id: "http",
					durationMs: FAST_MS,
					data: {"graphql.operation_name": "spaces"},
				}),
				span({
					span_id: "sql1",
					parent_span_id: "gql",
					data: {"db.system": "postgresql", "db.statement": "SELECT 1"},
				}),
				span({
					span_id: "sql2",
					parent_span_id: "gql",
					data: {"db.system": "postgresql", "db.statement": "SELECT 2"},
				}),
			]),
		)

		const kept = result.spans?.map((s) => s.span_id)
		expect(kept).toEqual(["http", "gql"])
	})

	it("keeps db spans under a slow graphql ancestor", () => {
		const result = filterFastGraphqlDbSpans(
			event([
				span({span_id: "http", data: {"http.method": "POST"}}),
				span({
					span_id: "gql",
					parent_span_id: "http",
					durationMs: SLOW_MS,
					data: {"graphql.operation_name": "spaces"},
				}),
				span({
					span_id: "sql1",
					parent_span_id: "gql",
					data: {"db.system": "postgresql", "db.statement": "SELECT 1"},
				}),
			]),
		)

		const kept = result.spans?.map((s) => s.span_id)
		expect(kept).toEqual(["http", "gql", "sql1"])
	})

	it("keeps db spans with no graphql ancestor (REST paths)", () => {
		const result = filterFastGraphqlDbSpans(
			event([
				span({span_id: "http", data: {"http.method": "GET"}}),
				span({
					span_id: "sql1",
					parent_span_id: "http",
					data: {"db.system": "postgresql", "db.statement": "SELECT 1"},
				}),
			]),
		)

		const kept = result.spans?.map((s) => s.span_id)
		expect(kept).toEqual(["http", "sql1"])
	})

	it("filters per graphql subtree in a batched transaction", () => {
		const result = filterFastGraphqlDbSpans(
			event([
				span({span_id: "http", data: {"http.method": "POST"}}),
				span({
					span_id: "gqlFast",
					parent_span_id: "http",
					durationMs: FAST_MS,
					data: {"graphql.operation_name": "profile"},
				}),
				span({
					span_id: "gqlSlow",
					parent_span_id: "http",
					durationMs: SLOW_MS,
					data: {"graphql.operation_name": "entities"},
				}),
				span({
					span_id: "sqlA",
					parent_span_id: "gqlFast",
					data: {"db.system": "postgresql", "db.statement": "SELECT A"},
				}),
				span({
					span_id: "sqlB",
					parent_span_id: "gqlSlow",
					data: {"db.system": "postgresql", "db.statement": "SELECT B"},
				}),
			]),
		)

		const kept = result.spans?.map((s) => s.span_id)
		expect(kept).toEqual(["http", "gqlFast", "gqlSlow", "sqlB"])
	})

	it("drops db spans nested transitively under a fast graphql span", () => {
		// e.g. pg span parented to an Effect span that is parented to the graphql span
		const result = filterFastGraphqlDbSpans(
			event([
				span({span_id: "http", data: {"http.method": "POST"}}),
				span({
					span_id: "gql",
					parent_span_id: "http",
					durationMs: FAST_MS,
					data: {"graphql.operation_name": "spaces"},
				}),
				span({span_id: "effect", parent_span_id: "gql", data: {"effect.span": "queries.getProfile"}}),
				span({
					span_id: "sql",
					parent_span_id: "effect",
					data: {"db.system": "postgresql", "db.statement": "SELECT 1"},
				}),
			]),
		)

		const kept = result.spans?.map((s) => s.span_id)
		expect(kept).toEqual(["http", "gql", "effect"])
	})

	it("is a no-op when no graphql spans are present", () => {
		const input = event([
			span({span_id: "http", data: {"http.method": "GET"}}),
			span({
				span_id: "sql",
				parent_span_id: "http",
				data: {"db.system": "postgresql", "db.statement": "SELECT 1"},
			}),
		])
		const result = filterFastGraphqlDbSpans(input)
		expect(result.spans).toBe(input.spans)
	})

	it("is a no-op when every graphql span is slow", () => {
		const input = event([
			span({span_id: "http", data: {"http.method": "POST"}}),
			span({
				span_id: "gql",
				parent_span_id: "http",
				durationMs: SLOW_MS,
				data: {"graphql.operation_name": "spaces"},
			}),
			span({
				span_id: "sql",
				parent_span_id: "gql",
				data: {"db.system": "postgresql", "db.statement": "SELECT 1"},
			}),
		])
		const result = filterFastGraphqlDbSpans(input)
		expect(result.spans?.map((s) => s.span_id)).toEqual(["http", "gql", "sql"])
	})
})
