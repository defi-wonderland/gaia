/**
 * Entity-diff endpoint coverage: every value type and block type across
 * ADD / EDIT / REMOVE.
 *
 * These are pure-function unit tests over `diff.ts` (`diffValues`, `diffBlocks`)
 * — the code path behind `GET /versioned/entities/:id/diff`. The existing
 * `diff.test.ts` only exercised TEXT / INT64 / BOOL for values and a partial
 * set of block changes; the integration suite covers all 13 value types only
 * in *snapshots*, not in *diffs*. This file closes the per-type diff gap at the
 * unit level (no DB required, always runs).
 *
 * The proposal-diff endpoint's own value deserialization is covered separately
 * in `proposal-value-types.test.ts`.
 */

import {SystemIds} from "@graphprotocol/grc-20"
import {Effect} from "effect"
import {describe, expect, it} from "vitest"
import {type NormalizedUuid, normalizeUuid} from "../../utils/uuid"
import {diffBlocks, diffValues} from "../diff"
import type {BlockSnapshot, SimpleValueChange, TextValueChange, VersionedValue} from "../types"

const nuuid = (s: string) => s as NormalizedUuid
const norm = normalizeUuid
const run = <A>(effect: Effect.Effect<A, never, never>): A => Effect.runSync(effect)

const PROP = "prop-1"
const SPACE = "space-1"

/** Build a VersionedValue carrying a single typed column. */
function value(col: Partial<VersionedValue>): VersionedValue {
	return {propertyId: nuuid(PROP), spaceId: nuuid(SPACE), ...col}
}

/**
 * Each value type with a `before` and a (different) `after` representation,
 * plus the string form `diffValues` is expected to surface in a SimpleValueChange.
 * TEXT is handled separately (it produces a TextValueChange with `diff` chunks).
 */
const SIMPLE_TYPES: Array<{
	type: SimpleValueChange["type"]
	before: Partial<VersionedValue>
	after: Partial<VersionedValue>
	beforeStr: string
	afterStr: string
}> = [
	{type: "BOOL", before: {boolean: false}, after: {boolean: true}, beforeStr: "false", afterStr: "true"},
	{type: "INT64", before: {integer: 10}, after: {integer: 20}, beforeStr: "10", afterStr: "20"},
	{type: "FLOAT64", before: {float: 1.5}, after: {float: 2.5}, beforeStr: "1.5", afterStr: "2.5"},
	{type: "DECIMAL", before: {decimal: "1.10"}, after: {decimal: "2.20"}, beforeStr: "1.10", afterStr: "2.20"},
	{type: "BYTES", before: {bytes: "AAA="}, after: {bytes: "QkJC"}, beforeStr: "AAA=", afterStr: "QkJC"},
	{
		type: "DATE",
		before: {date: "2024-01-15"},
		after: {date: "2024-02-20"},
		beforeStr: "2024-01-15",
		afterStr: "2024-02-20",
	},
	{type: "TIME", before: {time: "14:30:00"}, after: {time: "09:15:00"}, beforeStr: "14:30:00", afterStr: "09:15:00"},
	{
		type: "DATETIME",
		before: {datetime: "2024-01-15T14:30:00Z"},
		after: {datetime: "2024-02-20T09:15:00Z"},
		beforeStr: "2024-01-15T14:30:00Z",
		afterStr: "2024-02-20T09:15:00Z",
	},
	{
		type: "SCHEDULE",
		before: {schedule: {rrule: "FREQ=DAILY"}},
		after: {schedule: {rrule: "FREQ=WEEKLY"}},
		beforeStr: JSON.stringify({rrule: "FREQ=DAILY"}),
		afterStr: JSON.stringify({rrule: "FREQ=WEEKLY"}),
	},
	{type: "POINT", before: {point: "1,2"}, after: {point: "3,4"}, beforeStr: "1,2", afterStr: "3,4"},
	{type: "RECT", before: {rect: "1,2,3,4"}, after: {rect: "5,6,7,8"}, beforeStr: "1,2,3,4", afterStr: "5,6,7,8"},
	{
		type: "EMBEDDING",
		before: {embedding: [0.1, 0.2]},
		after: {embedding: [0.3, 0.4]},
		beforeStr: JSON.stringify([0.1, 0.2]),
		afterStr: JSON.stringify([0.3, 0.4]),
	},
]

describe("diffValues — TEXT (ADD/EDIT/REMOVE)", () => {
	it("ADD", () => {
		const result = run(diffValues([], [value({text: "hello"})]))
		const c = result[0] as TextValueChange
		expect(c.type).toBe("TEXT")
		expect(c.before).toBeNull()
		expect(c.after).toBe("hello")
		expect(c.diff).toContainEqual({value: "hello", added: true})
	})
	it("EDIT", () => {
		const result = run(diffValues([value({text: "hello world"})], [value({text: "hello there"})]))
		const c = result[0] as TextValueChange
		expect(c.type).toBe("TEXT")
		expect(c.before).toBe("hello world")
		expect(c.after).toBe("hello there")
		expect(c.diff).toContainEqual({value: "world", removed: true})
		expect(c.diff).toContainEqual({value: "there", added: true})
	})
	it("REMOVE", () => {
		const result = run(diffValues([value({text: "hello"})], []))
		const c = result[0] as TextValueChange
		expect(c.type).toBe("TEXT")
		expect(c.before).toBe("hello")
		expect(c.after).toBeNull()
	})
})

describe.each(SIMPLE_TYPES)("diffValues — $type (ADD/EDIT/REMOVE)", ({type, before, after, beforeStr, afterStr}) => {
	it("ADD: before null → after set", () => {
		const result = run(diffValues([], [value(after)]))
		expect(result).toHaveLength(1)
		const c = result[0] as SimpleValueChange
		expect(c.type).toBe(type)
		expect(c.before).toBeNull()
		expect(c.after).toBe(afterStr)
		// Simple (non-text) changes must not carry text-diff chunks.
		expect("diff" in c).toBe(false)
	})

	it("EDIT: before set → after changed", () => {
		const result = run(diffValues([value(before)], [value(after)]))
		expect(result).toHaveLength(1)
		const c = result[0] as SimpleValueChange
		expect(c.type).toBe(type)
		expect(c.before).toBe(beforeStr)
		expect(c.after).toBe(afterStr)
	})

	it("REMOVE: before set → after null", () => {
		const result = run(diffValues([value(before)], []))
		expect(result).toHaveLength(1)
		const c = result[0] as SimpleValueChange
		expect(c.type).toBe(type)
		expect(c.before).toBe(beforeStr)
		expect(c.after).toBeNull()
	})

	it("NO CHANGE: identical value omitted", () => {
		expect(run(diffValues([value(after)], [value(after)]))).toEqual([])
	})
})

describe("diffValues — value type changed across versions", () => {
	it("INT64 → TEXT is reported as a TEXT change", () => {
		const result = run(diffValues([value({integer: 42})], [value({text: "now text"})]))
		expect(result).toHaveLength(1)
		const c = result[0] as TextValueChange
		expect(c.type).toBe("TEXT")
		expect(c.before).toBe("42")
		expect(c.after).toBe("now text")
	})

	it("TEXT → INT64 is reported as a TEXT change", () => {
		const result = run(diffValues([value({text: "was text"})], [value({integer: 7})]))
		expect(result).toHaveLength(1)
		const c = result[0] as TextValueChange
		expect(c.type).toBe("TEXT")
		expect(c.before).toBe("was text")
		expect(c.after).toBe("7")
	})
})

// =============================================================================
// Block coverage gaps: imageBlock REMOVE, dataBlock EDIT + REMOVE
// (diff.test.ts already covers textBlock ADD/EDIT/REMOVE, imageBlock ADD/EDIT,
//  dataBlock ADD.)
// =============================================================================

function makeImageBlock(id: string, url: string | null): BlockSnapshot {
	return {
		id: nuuid(id),
		values: url ? [value2(norm(SystemIds.IMAGE_URL_PROPERTY), url)] : [],
		relations: [typeRelation(id, norm(SystemIds.IMAGE_BLOCK))],
	}
}
function makeDataBlock(id: string, name: string | null): BlockSnapshot {
	return {
		id: nuuid(id),
		values: name ? [value2(norm(SystemIds.NAME_PROPERTY), name)] : [],
		relations: [typeRelation(id, norm(SystemIds.DATA_BLOCK))],
	}
}
function value2(propertyId: string, text: string): VersionedValue {
	return {propertyId: nuuid(propertyId), spaceId: nuuid(SPACE), text}
}
function typeRelation(blockId: string, toEntityId: string) {
	return {
		relationId: nuuid(`${blockId}-type`),
		typeId: nuuid(norm(SystemIds.TYPES_PROPERTY)),
		fromEntityId: nuuid(blockId),
		toEntityId: nuuid(toEntityId),
		spaceId: nuuid(SPACE),
	}
}

describe("diffBlocks — coverage gaps", () => {
	it("imageBlock REMOVE", () => {
		const before = [makeImageBlock("img-1", "https://example.com/a.png")]
		const result = run(diffBlocks(before, []))
		expect(result).toHaveLength(1)
		expect(result[0]).toMatchObject({
			type: "imageBlock",
			before: "https://example.com/a.png",
			after: null,
		})
	})

	it("dataBlock EDIT (name changed)", () => {
		const before = [makeDataBlock("data-1", "Old Name")]
		const after = [makeDataBlock("data-1", "New Name")]
		const result = run(diffBlocks(before, after))
		expect(result).toHaveLength(1)
		expect(result[0]).toMatchObject({type: "dataBlock", before: "Old Name", after: "New Name"})
	})

	it("dataBlock REMOVE", () => {
		const before = [makeDataBlock("data-1", "Gone")]
		const result = run(diffBlocks(before, []))
		expect(result).toHaveLength(1)
		expect(result[0]).toMatchObject({type: "dataBlock", before: "Gone", after: null})
	})
})
