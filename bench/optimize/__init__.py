"""GEPA optimizer lane over the Gym.

OpenAgentsInc/openagents#123. Offline Python tier: wrap upstream `gepa`
around the Harbor suite runner, emit `openagents.coder_candidate.v1`
artifacts. Optimizer output is a candidate, never a deployment (ledger O1).
"""

from .candidate import CODER_CANDIDATE_SCHEMA, candidate_id_of, parse_candidate
from .metric import TOKEN_PENALTY_PER_MILLION, WALL_PENALTY_PER_HOUR, score_job

__all__ = [
    "CODER_CANDIDATE_SCHEMA",
    "TOKEN_PENALTY_PER_MILLION",
    "WALL_PENALTY_PER_HOUR",
    "candidate_id_of",
    "parse_candidate",
    "score_job",
]
