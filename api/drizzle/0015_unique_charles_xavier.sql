CREATE TABLE "votes" (
	"id" serial PRIMARY KEY NOT NULL,
	"voter_id" uuid NOT NULL,
	"object_id" uuid NOT NULL,
	"object_type" smallint NOT NULL,
	"space_id" uuid NOT NULL,
	"vote" smallint NOT NULL,
	"block_number" bigint NOT NULL,
	"block_timestamp" timestamp with time zone NOT NULL
);
--> statement-breakpoint
ALTER TABLE "raw_actions" DISABLE ROW LEVEL SECURITY;--> statement-breakpoint
DROP TABLE "raw_actions" CASCADE;--> statement-breakpoint
ALTER TABLE "user_votes" DROP CONSTRAINT "user_votes_user_entity_object_type_space_unique";--> statement-breakpoint
DROP INDEX "idx_user_votes_user_entity_object_type_space";--> statement-breakpoint
ALTER TABLE "user_votes" ADD COLUMN "user_address" varchar(42) NOT NULL;--> statement-breakpoint
CREATE INDEX "idx_votes_voter_id" ON "votes" USING btree ("voter_id");--> statement-breakpoint
CREATE INDEX "idx_votes_object_id" ON "votes" USING btree ("object_id");--> statement-breakpoint
CREATE INDEX "idx_votes_space_id" ON "votes" USING btree ("space_id");--> statement-breakpoint
CREATE INDEX "idx_votes_block_number" ON "votes" USING btree ("block_number");--> statement-breakpoint
CREATE INDEX "idx_user_votes_user_entity_object_type_space" ON "user_votes" USING btree ("user_address","object_id","object_type","space_id");--> statement-breakpoint
ALTER TABLE "user_votes" DROP COLUMN "user_id";--> statement-breakpoint
ALTER TABLE "user_votes" ADD CONSTRAINT "user_votes_user_entity_object_type_space_unique" UNIQUE("user_address","object_id","object_type","space_id");