/**
 * Proposal-diff endpoint coverage: every value type across ADD / EDIT / REMOVE,
 * exercised through the proposal path's own deserialization.
 *
 * `GET /versioned/proposals/:id/diff` does NOT reuse the entity-diff value
 * handling. It first decodes a GRC-20 edit blob and converts each op value to a
 * `VersionedValue` via `propertyValueToVersionedValue` (proposal-diff.ts), then
 * feeds the result to `diffEntitySnapshots` → `diffValues`. That conversion is
 * per-type and non-trivial (decimal mantissa/exponent formatting, bytes base64,
 * point/rect coordinate flattening, int64 bigint handling). The existing tests
 * only covered text/bool/int/float (edit-flow integration) and point/rect
 * serialization (proposal-diff-point-value). This file covers all 13 types at
 * the unit level — no DB required.
 *
 * GRC-20 `Value` shapes are taken from @geoprotocol/grc-20 types/value.d.ts.
 */

import {Effect} from "effect"
import {describe, expect, it} from "vitest"
import {normalizeUuid} from "../../utils/uuid"
import {diffValues} from "../diff"
import {propertyValueToVersionedValue} from "../proposal-diff"
import type {SimpleValueChange, TextValueChange, VersionedValue} from "../types"

const run = <A>(effect: Effect.Effect<A, never, never>): A => Effect.runSync(effect)
const SPACE = normalizeUuid("20000000-0001-4000-8000-000000000001")
const PROP = "20000000-0004-4000-8000-000000000050"

/** Decoded GRC-20 values carry fields the SDK type doesn't always surface
 *  (decimal mantissa/exponent, point/rect coords), so accept a loose shape. */
type DecodedValue = {type: string; [k: string]: unknown}

function toVV(value: DecodedValue): VersionedValue {
	return propertyValueToVersionedValue({property: PROP as never, value: value as never}, SPACE)
}

/** Serialize a value the way the proposal diff surfaces it: ADD diff `after`. */
function addAfter(value: DecodedValue): string | null {
	const result = run(diffValues([], [toVV(value)]))
	return result.length ? ((result[0] as SimpleValueChange | TextValueChange).after ?? null) : null
}

const b64 = (bytes: number[]) => Buffer.from(Uint8Array.from(bytes)).toString("base64")

// -----------------------------------------------------------------------------
// All 13 types through ADD / EDIT / REMOVE (representative, well-behaved values).
// INT64 large-value, DECIMAL formatting, and EMBEDDING get dedicated blocks below.
// -----------------------------------------------------------------------------

const TYPES: Array<{
	name: string
	before: DecodedValue
	after: DecodedValue
	beforeStr: string
	afterStr: string
	isText?: boolean
}> = [
	{
		name: "TEXT",
		before: {type: "text", value: "alpha"},
		after: {type: "text", value: "beta"},
		beforeStr: "alpha",
		afterStr: "beta",
		isText: true,
	},
	{
		name: "BOOL",
		before: {type: "bool", value: false},
		after: {type: "bool", value: true},
		beforeStr: "false",
		afterStr: "true",
	},
	{
		name: "INT64",
		before: {type: "int64", value: 10n},
		after: {type: "int64", value: 20n},
		beforeStr: "10",
		afterStr: "20",
	},
	{
		name: "FLOAT64",
		before: {type: "float64", value: 1.5},
		after: {type: "float64", value: 2.5},
		beforeStr: "1.5",
		afterStr: "2.5",
	},
	{
		name: "DECIMAL",
		before: {type: "decimal", exponent: -2, mantissa: {type: "i64", value: 314n}},
		after: {type: "decimal", exponent: -2, mantissa: {type: "i64", value: 271n}},
		beforeStr: "3.14",
		afterStr: "2.71",
	},
	{
		name: "BYTES",
		before: {type: "bytes", value: Uint8Array.from([1, 2, 3])},
		after: {type: "bytes", value: Uint8Array.from([4, 5, 6])},
		beforeStr: b64([1, 2, 3]),
		afterStr: b64([4, 5, 6]),
	},
	{
		name: "DATE",
		before: {type: "date", value: "2024-01-15"},
		after: {type: "date", value: "2024-02-20"},
		beforeStr: "2024-01-15",
		afterStr: "2024-02-20",
	},
	{
		name: "TIME",
		before: {type: "time", value: "14:30:00"},
		after: {type: "time", value: "09:15:00"},
		beforeStr: "14:30:00",
		afterStr: "09:15:00",
	},
	{
		name: "DATETIME",
		before: {type: "datetime", value: "2024-01-15T14:30:00Z"},
		after: {type: "datetime", value: "2024-02-20T09:15:00Z"},
		beforeStr: "2024-01-15T14:30:00Z",
		afterStr: "2024-02-20T09:15:00Z",
	},
	{
		name: "SCHEDULE",
		before: {type: "schedule", value: "FREQ=DAILY"},
		after: {type: "schedule", value: "FREQ=WEEKLY"},
		beforeStr: JSON.stringify("FREQ=DAILY"),
		afterStr: JSON.stringify("FREQ=WEEKLY"),
	},
	{
		name: "POINT",
		before: {type: "point", lat: 1, lon: 2},
		after: {type: "point", lat: 3, lon: 4},
		beforeStr: "1,2",
		afterStr: "3,4",
	},
	{
		name: "RECT",
		before: {type: "rect", minLat: 1, minLon: 2, maxLat: 3, maxLon: 4},
		after: {type: "rect", minLat: 5, minLon: 6, maxLat: 7, maxLon: 8},
		beforeStr: "1,2,3,4",
		afterStr: "5,6,7,8",
	},
]

describe.each(TYPES)("proposal value path — $name (ADD/EDIT/REMOVE)", ({before, after, beforeStr, afterStr}) => {
	it("ADD", () => {
		const result = run(diffValues([], [toVV(after)]))
		expect(result).toHaveLength(1)
		expect(result[0]?.before).toBeNull()
		expect(result[0]?.after).toBe(afterStr)
	})

	it("EDIT", () => {
		const result = run(diffValues([toVV(before)], [toVV(after)]))
		expect(result).toHaveLength(1)
		expect(result[0]?.before).toBe(beforeStr)
		expect(result[0]?.after).toBe(afterStr)
	})

	it("REMOVE", () => {
		const result = run(diffValues([toVV(before)], []))
		expect(result).toHaveLength(1)
		expect(result[0]?.before).toBe(beforeStr)
		expect(result[0]?.after).toBeNull()
	})
})

// -----------------------------------------------------------------------------
// DECIMAL string formatting (proposal-diff.ts builds the string from
// mantissa/exponent). Covers positive exponent, fractional, sub-1 leading
// zeros, and negative values.
// -----------------------------------------------------------------------------

describe("proposal value path — DECIMAL formatting", () => {
	const dec = (mantissa: bigint, exponent: number) =>
		toVV({type: "decimal", exponent, mantissa: {type: "i64", value: mantissa}}).decimal

	it("positive exponent appends zeros (123 e2 = 12300)", () => {
		expect(dec(123n, 2)).toBe("12300")
	})
	it("zero exponent is the integer itself (42 e0 = 42)", () => {
		expect(dec(42n, 0)).toBe("42")
	})
	it("negative exponent inserts a decimal point (123456 e-3 = 123.456)", () => {
		expect(dec(123456n, -3)).toBe("123.456")
	})
	it("sub-1 value pads leading zeros (5 e-2 = 0.05)", () => {
		expect(dec(5n, -2)).toBe("0.05")
	})
	it("negative fractional value (-12345 e-2 = -123.45)", () => {
		expect(dec(-12345n, -2)).toBe("-123.45")
	})
	it("negative sub-1 value (-5 e-2 = -0.05)", () => {
		expect(dec(-5n, -2)).toBe("-0.05")
	})
})

// -----------------------------------------------------------------------------
// INT64 precision: int64 spans up to 9.2e18, far beyond JS Number's safe
// integer (2^53). The decimal path preserves big values via string math; int64
// must not silently lose precision.
// -----------------------------------------------------------------------------

describe("proposal value path — INT64 precision", () => {
	it("preserves a value just above 2^53", () => {
		const big = 9007199254740993n // 2^53 + 1
		expect(addAfter({type: "int64", value: big})).toBe(big.toString())
	})

	it("preserves a near-max int64", () => {
		const big = 9223372036854775807n // 2^63 - 1
		expect(addAfter({type: "int64", value: big})).toBe(big.toString())
	})
})

// -----------------------------------------------------------------------------
// EMBEDDING: GRC-20 decodes embeddings as {subType, dims, data}, with no
// `value` field — reading `value.value` silently dropped the content. The
// proposal path must mirror the kg-indexer's stored shape ({sub_type, dims,
// data: hex}) so a proposal diff matches the snapshot/live representation
// instead of showing a spurious or quote-wrapped change.
// -----------------------------------------------------------------------------

describe("proposal value path — EMBEDDING", () => {
	const data = Uint8Array.from([1, 2, 3, 4, 5, 6, 7, 8])

	it("mirrors the kg-indexer snapshot shape: {sub_type, dims, data: hex}", () => {
		const vv = toVV({type: "embedding", subType: 0, dims: 2, data})
		expect(vv.embedding).toEqual({sub_type: "Float32", dims: 2, data: "0102030405060708"})
	})

	it("does not diff against the snapshot representation regardless of JSONB key order", () => {
		const proposed = toVV({type: "embedding", subType: 0, dims: 2, data})
		// The snapshot path returns the JSONB object with keys in an arbitrary order.
		const snapshot: VersionedValue = {
			propertyId: proposed.propertyId,
			spaceId: proposed.spaceId,
			embedding: {data: "0102030405060708", sub_type: "Float32", dims: 2},
		}
		expect(run(diffValues([snapshot], [proposed]))).toEqual([])
	})

	it("serializes without extra quote-wrapping", () => {
		const after = addAfter({type: "embedding", subType: 1, dims: 3, data: Uint8Array.from([10, 20, 30])})
		expect(after?.startsWith('"')).toBe(false)
		expect(after).toContain("Int8")
	})
})

// -----------------------------------------------------------------------------
// DECIMAL big mantissa: mantissas too large for i64 decode as {type:"big",
// bytes}, which has no `value` field. The proposal path must handle it without
// throwing.
// -----------------------------------------------------------------------------

describe("proposal value path — DECIMAL big mantissa", () => {
	it("does not throw on a big-variant mantissa", () => {
		expect(() =>
			toVV({
				type: "decimal",
				exponent: 0,
				mantissa: {type: "big", bytes: Uint8Array.from([1, 2, 3, 4, 5, 6, 7, 8, 9])},
			}),
		).not.toThrow()
	})
})
