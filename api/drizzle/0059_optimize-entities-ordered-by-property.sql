-- GEO-675: optimize entities_ordered_by_property (sorted table/collection views).
--
-- The previous function materialized every entity that has the sort property in the
-- space, sorted them with a non-indexable ORDER BY CASE, and only then let PostGraphile
-- apply the type filter + page limit. This rewrites it to be filter-first, index-ordered
-- and limit-early:
--   * resolve the typed sort column for the property's data type,
--   * build the WHERE clause CONDITIONALLY (only emit predicates for non-null params,
--     no "param IS NULL OR ..." guards) so the planner can use indexes and turn the type
--     EXISTS into a semi-join that drives from the selective type set,
--   * order by the typed column directly (index-driven, no CASE, no DISTINCT).
--
-- A new optional `type_ids uuid[]` param pushes the type filter inside the query. It is
-- backward compatible: callers that omit it (today's clients) get the previous behavior
-- (no type filter), so the backend can ship ahead of the frontend query-builder change.
--
-- Depends on the per-type partial composite indexes created at the bottom of this file.

DROP FUNCTION IF EXISTS public.entities_ordered_by_property(uuid, uuid, sort_order, text);
--> statement-breakpoint
DROP FUNCTION IF EXISTS public.entities_ordered_by_property(uuid, uuid, sort_order, text, uuid);
--> statement-breakpoint
CREATE OR REPLACE FUNCTION public.entities_ordered_by_property(
  property_id uuid,
  space_id uuid DEFAULT NULL,
  sort_direction sort_order DEFAULT 'ASC',
  data_type text DEFAULT NULL,
  type_ids uuid[] DEFAULT NULL   -- NULL/empty = no type filter; multiple = match ANY (OR)
)
RETURNS SETOF entities AS $fn$
DECLARE
  resolved_type text;
  sort_expr     text;
  null_pred     text;
  dir           text;
  sql           text;
BEGIN
  IF data_type IS NOT NULL THEN
    resolved_type := lower(data_type);
  ELSE
    SELECT lower(v."text")
      INTO resolved_type
      FROM "values" v
      JOIN relations r ON r.to_entity_id = v.entity_id
     WHERE r.from_entity_id = entities_ordered_by_property.property_id
       AND r.type_id    = '6d29d578-49bb-4959-baf7-2cc696b1671a' -- DATA_TYPE_PROPERTY_ID
       AND v.property_id = 'a126ca53-0c8e-48d5-b888-82c734c38935' -- NAME_PROPERTY_ID
     LIMIT 1;
  END IF;

  CASE resolved_type
    WHEN 'text'     THEN sort_expr := 'left(v."text", 1024)'; null_pred := 'v."text" IS NOT NULL AND length(btrim(v."text")) > 0';
    WHEN 'integer'  THEN sort_expr := 'v.integer';            null_pred := 'v.integer IS NOT NULL';
    WHEN 'float'    THEN sort_expr := 'v.float';              null_pred := 'v.float IS NOT NULL';
    WHEN 'decimal'  THEN sort_expr := 'v."decimal"';          null_pred := 'v."decimal" IS NOT NULL';
    WHEN 'boolean'  THEN sort_expr := 'v.boolean';            null_pred := 'v.boolean IS NOT NULL';
    WHEN 'date'     THEN sort_expr := 'v.date';               null_pred := 'v.date IS NOT NULL AND length(btrim(v.date)) > 0';
    WHEN 'time'     THEN sort_expr := 'v."time"';             null_pred := 'v."time" IS NOT NULL AND length(btrim(v."time")) > 0';
    WHEN 'datetime' THEN sort_expr := 'v.datetime';           null_pred := 'v.datetime IS NOT NULL AND length(btrim(v.datetime)) > 0';
    WHEN 'point'    THEN sort_expr := 'v.point';              null_pred := 'v.point IS NOT NULL AND length(btrim(v.point)) > 0';
    ELSE
      RAISE EXCEPTION 'entities_ordered_by_property: unsupported sortable data_type %', resolved_type;
  END CASE;

  dir := CASE WHEN sort_direction::text = 'DESC' THEN 'DESC' ELSE 'ASC' END;

  sql := format(
    'SELECT e.* FROM "values" v JOIN entities e ON e.id = v.entity_id WHERE v.property_id = %L AND %s',
    property_id, null_pred);

  IF space_id IS NOT NULL THEN
    sql := sql || format(' AND v.space_id = %L', space_id);
  END IF;

  -- Match entities that have ANY of the requested types (OR semantics), mirroring the
  -- app's multi-type filter. A single type is just an array of length one. The exact
  -- filter (AND of types, value predicates, etc.) is still applied by PostGraphile as a
  -- residual over this output, so passing a NECESSARY type superset here is sufficient.
  IF type_ids IS NOT NULL AND cardinality(type_ids) > 0 THEN
    sql := sql || format(
      ' AND EXISTS (SELECT 1 FROM relations r WHERE r.from_entity_id = v.entity_id AND r.type_id = %L AND r.to_entity_id = ANY(%L::uuid[]))',
      '8f151ba4-de20-4e3c-9cb4-99ddf96f48f1', type_ids); -- TYPES_PROPERTY
  END IF;

  sql := sql || ' ORDER BY ' || sort_expr || ' ' || dir;

  RETURN QUERY EXECUTE sql;
END;
$fn$ LANGUAGE plpgsql STABLE;
--> statement-breakpoint
-- Per-type partial composite sort indexes. (property_id, space_id, <typed column>) lets
-- the function filter by property+space and read rows already in sorted order; one btree
-- serves both ASC and DESC via backward scan. The text index uses a bounded prefix because
-- raw values.text can exceed btree's row-size limit.
--
-- NOTE FOR DEPLOY: on a large `values` table, prefer creating these CONCURRENTLY out of
-- band before running this migration (the statements below are IF NOT EXISTS, so they
-- then no-op). Plain CREATE INDEX takes a write lock on `values` for the duration of the
-- build.
CREATE INDEX IF NOT EXISTS values_sort_text_idx     ON "values"(property_id, space_id, left("text", 1024)) WHERE "text"   IS NOT NULL;
--> statement-breakpoint
CREATE INDEX IF NOT EXISTS values_sort_integer_idx  ON "values"(property_id, space_id, integer)   WHERE integer  IS NOT NULL;
--> statement-breakpoint
CREATE INDEX IF NOT EXISTS values_sort_float_idx    ON "values"(property_id, space_id, float)     WHERE float    IS NOT NULL;
--> statement-breakpoint
CREATE INDEX IF NOT EXISTS values_sort_decimal_idx  ON "values"(property_id, space_id, "decimal") WHERE "decimal" IS NOT NULL;
--> statement-breakpoint
CREATE INDEX IF NOT EXISTS values_sort_boolean_idx  ON "values"(property_id, space_id, boolean)   WHERE boolean  IS NOT NULL;
--> statement-breakpoint
CREATE INDEX IF NOT EXISTS values_sort_date_idx     ON "values"(property_id, space_id, date)      WHERE date     IS NOT NULL;
--> statement-breakpoint
CREATE INDEX IF NOT EXISTS values_sort_time_idx     ON "values"(property_id, space_id, "time")    WHERE "time"   IS NOT NULL;
--> statement-breakpoint
CREATE INDEX IF NOT EXISTS values_sort_datetime_idx ON "values"(property_id, space_id, datetime)  WHERE datetime IS NOT NULL;
--> statement-breakpoint
CREATE INDEX IF NOT EXISTS values_sort_point_idx    ON "values"(property_id, space_id, point)     WHERE point    IS NOT NULL;
