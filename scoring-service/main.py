"""Main entry point for the scoring service."""

import logging
import os
import sys

from dotenv import load_dotenv

from src.algorithm.models import RankingConfig
from src.algorithm.scoring import RankingEngine
from src.scoring_data_provider import ScoringDataProvider
from src.scoring_data_provider.scoring_data_provider import ScoringData
from src.scoring_data_writer import ScoringDataWriter

logger = logging.getLogger(__name__)


class ScoringPipeline:
    """Scoring pipeline."""

    def __init__(self, database_url: str):
        self.database_url = database_url
        self.scoring_data: ScoringData | None = None

    def run(self) -> tuple[int, int]:
        """Run the full scoring pipeline."""
        logger.info("Starting scoring pipeline")

        self._fetch_data()
        self._rank_spaces()
        self._rank_entities()
        self._write_scores()

        entity_count = len(self.scoring_data.entities)
        space_count = len(self.scoring_data.spaces)
        logger.info("Scoring pipeline completed: %d entities, %d spaces", entity_count, space_count)

        return entity_count, space_count

    def _fetch_data(self) -> None:
        """Fetch data from the database."""
        logger.info("Fetching scoring data from database")
        provider = ScoringDataProvider(self.database_url)
        self.scoring_data = provider.fetch_all()
        logger.info(
            "Fetched %d entities, %d spaces, %d users, %d votes",
            len(self.scoring_data.entities),
            len(self.scoring_data.spaces),
            len(self.scoring_data.users),
            len(self.scoring_data.votes),
        )

    def _rank_spaces(self) -> None:
        """Rank spaces by their scores."""
        logger.info("Ranking spaces")
        config = RankingConfig()
        engine = RankingEngine(config)
        self.scoring_data.spaces = engine.rank_spaces(
            self.scoring_data.spaces,
            self.scoring_data.entities,
            self.scoring_data.users,
        )
        logger.info("Ranked %d spaces", len(self.scoring_data.spaces))

    def _rank_entities(self) -> None:
        """Rank entities by their scores."""
        logger.info("Ranking entities")
        config = RankingConfig()
        engine = RankingEngine(config)
        self.scoring_data.entities = engine.rank_entities(
            self.scoring_data.entities,
            self.scoring_data.votes,
            self.scoring_data.users,
            self.scoring_data.spaces,
        )
        logger.info("Ranked %d entities", len(self.scoring_data.entities))

    def _write_scores(self) -> None:
        """Write scores to the database."""
        logger.info("Writing scores to database")
        writer = ScoringDataWriter(self.database_url)
        writer.write_all(self.scoring_data.entities, self.scoring_data.spaces)
        logger.info("Scores written successfully")


def main() -> None:
    """Run the scoring pipeline."""
    load_dotenv()

    log_level = os.environ.get("LOG_LEVEL", "INFO").upper()
    logging.basicConfig(
        level=getattr(logging, log_level, logging.INFO),
        format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
        handlers=[logging.StreamHandler(sys.stdout)],
    )

    database_url = os.environ.get("DATABASE_URL")
    if not database_url:
        logger.error("DATABASE_URL environment variable is required")
        sys.exit(1)

    try:
        ScoringPipeline(database_url).run()
    except Exception:
        logger.exception("Scoring pipeline failed")
        sys.exit(1)


if __name__ == "__main__":
    main()
